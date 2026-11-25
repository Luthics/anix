//! External Provider adapter runner supporting persistent `coprocess` mode (v1.1 default,
//! reusing HTTP/TLS pools across turns) and legacy `oneshot` mode, with pure event-driven
//! stdout reading and exponential backoff retries (`1s / 2s / 4s`) (v1.1 §§6.3 & 9.2).

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{IrStream, StderrMode, ToolCall};
use os_agent_proto::provider::{ProviderEvent, ProviderMode, ProviderRequestEnvelope, TokenUsage};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::config::{AgentPaths, ProviderConfig};
use crate::fd::StderrTracker;
use crate::platform::{self, ChildWaitOutcome, posix};
use crate::signal::SignalState;
use crate::trace::Tracer;
use crate::trust::{self, TrustError};

const RETRY_DELAYS_SEC: &[u64] = &[1, 2, 4];
const COPROCESS_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error(transparent)]
    Trust(#[from] TrustError),
    #[error("provider `{provider}` timed out after {timeout_sec}s")]
    Timeout { provider: String, timeout_sec: u64 },
    #[error("provider `{provider}` truncated before producing output: {message}")]
    OutputTruncatedEmpty {
        provider: String,
        message: String,
        usage: Option<TokenUsage>,
    },
    #[error("interrupted by signal")]
    Interrupted(ExitCode),
    #[error("provider `{provider}` failed: {message}")]
    Failed {
        provider: String,
        message: String,
        usage: Option<TokenUsage>,
    },
}

impl ProviderError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Trust(_) => ExitCode::TempFail,
            Self::Timeout { .. } | Self::OutputTruncatedEmpty { .. } => ExitCode::TimeoutOrBudget,
            Self::Interrupted(code) => *code,
            Self::Failed { .. } => ExitCode::RuntimeError,
        }
    }

    pub fn usage(&self) -> Option<TokenUsage> {
        match self {
            Self::OutputTruncatedEmpty { usage, .. } | Self::Failed { usage, .. } => *usage,
            Self::Trust(_) | Self::Timeout { .. } | Self::Interrupted(_) => None,
        }
    }
}

/// Aggregated result of a completed Provider inference turn.
#[derive(Debug, Clone)]
pub struct ProviderTurnResult {
    pub text: String,
    pub reasoning: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<TokenUsage>,
    pub provider_opaque: Option<Value>,
    pub finish_reason: String,
}

/// Event-driven buffered line reader over a child's `stdout` pipe (zero polling).
struct BufferedStdoutReader {
    stdout: ChildStdout,
    buf: Vec<u8>,
}

enum ReadLineOutcome {
    Line(String),
    Eof,
    TimedOut,
    SignalWakeup,
}

impl BufferedStdoutReader {
    fn new(stdout: ChildStdout) -> Self {
        Self {
            stdout,
            buf: Vec::with_capacity(4096),
        }
    }

    fn read_line_until(
        &mut self,
        deadline: Instant,
        signals: &SignalState,
    ) -> io::Result<ReadLineOutcome> {
        loop {
            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let mut line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
                if line_bytes.last() == Some(&b'\n') {
                    line_bytes.pop();
                }
                if line_bytes.last() == Some(&b'\r') {
                    line_bytes.pop();
                }
                return Ok(ReadLineOutcome::Line(
                    String::from_utf8_lossy(&line_bytes).into_owned(),
                ));
            }

            if signals.termination_exit_code().is_some() {
                return Ok(ReadLineOutcome::SignalWakeup);
            }
            if Instant::now() >= deadline {
                return Ok(ReadLineOutcome::TimedOut);
            }

            match posix::poll_readable_or_signal(
                self.stdout.as_raw_fd(),
                signals.wakeup_fd(),
                Some(deadline),
            )? {
                posix::PollFdOutcome::TimedOut => return Ok(ReadLineOutcome::TimedOut),
                posix::PollFdOutcome::SignalWakeup => {
                    signals.drain_wakeup();
                    return Ok(ReadLineOutcome::SignalWakeup);
                }
                posix::PollFdOutcome::FdReady => {
                    let mut chunk = [0u8; 8192];
                    // SAFETY: `chunk` is a valid writable buffer for `read(2)`.
                    let n = unsafe {
                        libc::read(
                            self.stdout.as_raw_fd(),
                            chunk.as_mut_ptr().cast(),
                            chunk.len(),
                        )
                    };
                    if n < 0 {
                        let err = io::Error::last_os_error();
                        if err.kind() == io::ErrorKind::Interrupted {
                            continue;
                        }
                        return Err(err);
                    }
                    if n == 0 {
                        if !self.buf.is_empty() {
                            let rem = std::mem::take(&mut self.buf);
                            return Ok(ReadLineOutcome::Line(
                                String::from_utf8_lossy(&rem).into_owned(),
                            ));
                        }
                        return Ok(ReadLineOutcome::Eof);
                    }
                    self.buf.extend_from_slice(&chunk[..n as usize]);
                }
            }
        }
    }
}

struct CoprocessHandle {
    child: Child,
    pgid: libc::pid_t,
    stdin: Option<ChildStdin>,
    stdout: BufferedStdoutReader,
    stderr_buf: Arc<Mutex<String>>,
}

impl CoprocessHandle {
    fn spawn(
        bin_path: &Path,
        provider_cfg: &ProviderConfig,
        model: &str,
        api_key: &str,
        cwd: &Path,
    ) -> io::Result<Self> {
        let parent_pid = unsafe { libc::getpid() };
        let mut cmd = Command::new(bin_path);
        cmd.current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("AGENT_PROVIDER_BASE_URL", &provider_cfg.base_url)
            .env("AGENT_PROVIDER_API_KEY", api_key)
            .env(
                "AGENT_PROVIDER_TIMEOUT_SEC",
                provider_cfg.timeout_sec.to_string(),
            )
            .env("AGENT_MODEL", model)
            .env("AGENT_PROVIDER_MODE", ProviderMode::Coprocess.as_str());

        // SAFETY: `pre_exec` invokes only async-signal-safe syscalls to isolate PGID and set PDEATHSIG.
        unsafe {
            cmd.pre_exec(move || {
                if libc::setpgid(0, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                #[cfg(target_os = "linux")]
                {
                    crate::platform::linux::set_pdeathsig_sigkill(parent_pid)?;
                }
                #[cfg(not(target_os = "linux"))]
                let _ = parent_pid;
                Ok(())
            });
        }

        let mut child = cmd.spawn()?;
        let pgid = child.id() as libc::pid_t;
        let stdin = child.stdin.take();
        let stdout_pipe = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("coprocess stdout pipe missing"))?;
        let stderr_pipe = child.stderr.take();

        let stderr_buf = Arc::new(Mutex::new(String::new()));
        let stderr_clone = Arc::clone(&stderr_buf);
        thread::spawn(move || {
            let Some(mut pipe) = stderr_pipe else {
                return;
            };
            let mut chunk = [0u8; 4096];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut guard) = stderr_clone.lock() {
                            guard.push_str(&String::from_utf8_lossy(&chunk[..n]));
                            // Keep bounded recent stderr (last 16 KiB) to avoid unbounded growth in long `--inbox` sessions.
                            if guard.len() > 16 * 1024 {
                                let keep_from = guard.len() - 8 * 1024;
                                let mut idx = keep_from;
                                while !guard.is_char_boundary(idx) && idx < guard.len() {
                                    idx += 1;
                                }
                                guard.drain(..idx);
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            child,
            pgid,
            stdin,
            stdout: BufferedStdoutReader::new(stdout_pipe),
            stderr_buf,
        })
    }

    fn take_stderr_snapshot(&self) -> String {
        self.stderr_buf
            .lock()
            .map(|mut g| std::mem::take(&mut *g))
            .unwrap_or_default()
    }

    fn clear_stderr(&self) {
        if let Ok(mut g) = self.stderr_buf.lock() {
            g.clear();
        }
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn shutdown_graceful(mut self) {
        // Drop `stdin` to send EOF to the coprocess (v1.1 §9.2).
        drop(self.stdin.take());
        let deadline = Instant::now() + COPROCESS_SHUTDOWN_GRACE;
        match platform::wait_child_until(&mut self.child, Some(deadline), None) {
            Ok(ChildWaitOutcome::Exited(_)) => {}
            _ => {
                let _ = posix::terminate_child_process_group(
                    &mut self.child,
                    self.pgid,
                    Duration::from_secs(1),
                );
            }
        }
    }

    fn terminate_now(mut self) {
        drop(self.stdin.take());
        let _ = posix::terminate_child_process_group(
            &mut self.child,
            self.pgid,
            Duration::from_secs(3),
        );
    }
}

/// Session-scoped pool of persistent Provider coprocesses (v1.1 §9.2).
#[derive(Default)]
pub struct ProviderPool {
    coprocesses: BTreeMap<(String, PathBuf, String), CoprocessHandle>,
    next_req_id: u64,
}

impl std::fmt::Debug for ProviderPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderPool")
            .field("active_coprocesses", &self.coprocesses.len())
            .field("next_req_id", &self.next_req_id)
            .finish()
    }
}

impl ProviderPool {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_request_id(&mut self) -> String {
        self.next_req_id = self.next_req_id.saturating_add(1);
        format!("req_{}", self.next_req_id)
    }

    /// Gracefully close all persistent coprocesses (send stdin EOF and wait for clean exit 0).
    pub fn shutdown(&mut self) {
        let handles = std::mem::take(&mut self.coprocesses);
        for (_, handle) in handles {
            handle.shutdown_graceful();
        }
    }
}

impl Drop for ProviderPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct SingleAttemptFailure {
    code: String,
    message: String,
    retryable: bool,
    timed_out: bool,
    output_truncated_empty: bool,
    interrupted: Option<ExitCode>,
    provider_stderr: String,
    partial_text: String,
    partial_reasoning: String,
    last_delta_ts: Option<String>,
    usage: Option<TokenUsage>,
    elapsed_ms: u64,
}

/// Invoke the configured Provider adapter (in `coprocess` or `oneshot` mode) with
/// deterministic exponential backoff retries (`1s / 2s / 4s`) on retryable errors.
#[allow(clippy::too_many_arguments)]
pub fn invoke_provider(
    provider_cfg: &ProviderConfig,
    model: &str,
    stream: &IrStream,
    paths: &AgentPaths,
    stderr_mode: StderrMode,
    stream_to_stderr: bool,
    signals: &SignalState,
    pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> Result<ProviderTurnResult, ProviderError> {
    let base_dir = paths.project_agent_dir.as_deref().and_then(|d| d.parent());
    let bin_path = paths.resolve_component_bin(&provider_cfg.bin, base_dir);

    if provider_cfg.bin_from_project {
        trust::verify_trusted(&bin_path, paths)?;
    }

    let ir_jsonl = if provider_cfg.mode == ProviderMode::Oneshot || tracer.is_enabled() {
        stream.to_jsonl().map_err(|e| ProviderError::Failed {
            provider: provider_cfg.name.clone(),
            message: format!("failed to serialize Canonical IR: {e}"),
            usage: None,
        })?
    } else {
        String::new()
    };

    if tracer.is_enabled() {
        let mut wire = Map::new();
        wire.insert("provider".into(), Value::String(provider_cfg.name.clone()));
        wire.insert("model".into(), Value::String(model.to_owned()));
        wire.insert(
            "mode".into(),
            Value::String(provider_cfg.mode.as_str().into()),
        );
        wire.insert("ir_jsonl".into(), Value::String(ir_jsonl.clone()));
        tracer.emit_wire("wire_ir_request", wire);
    }

    let (api_key, _) = provider_cfg.resolve_api_key();

    let max_attempts = 1 + RETRY_DELAYS_SEC.len();
    let mut accumulated_failed_usage = TokenUsage::default();
    let mut saw_failed_usage = false;
    for attempt in 1..=max_attempts {
        if let Some(code) = signals.termination_exit_code() {
            return Err(ProviderError::Interrupted(code));
        }

        let mut req_fields = Map::new();
        req_fields.insert("provider".into(), Value::String(provider_cfg.name.clone()));
        req_fields.insert("model".into(), Value::String(model.to_owned()));
        req_fields.insert(
            "mode".into(),
            Value::String(provider_cfg.mode.as_str().into()),
        );
        req_fields.insert("attempt".into(), Value::from(attempt as u64));
        req_fields.insert("messages".into(), Value::from(stream.messages.len() as u64));
        tracer.emit_event("llm_request_start", req_fields);

        let attempt_result = match provider_cfg.mode {
            ProviderMode::Coprocess => run_coprocess_attempt(
                pool,
                &bin_path,
                provider_cfg,
                model,
                &api_key,
                stream,
                &paths.cwd,
                stderr_mode,
                stream_to_stderr,
                signals,
                stderr_tracker,
                tracer,
            ),
            ProviderMode::Oneshot => run_oneshot_attempt(
                &bin_path,
                provider_cfg,
                model,
                &api_key,
                &ir_jsonl,
                &paths.cwd,
                stderr_mode,
                stream_to_stderr,
                signals,
                stderr_tracker,
                tracer,
            ),
        };

        match attempt_result {
            Ok(result) => {
                let mut resp_fields = Map::new();
                resp_fields.insert("provider".into(), Value::String(provider_cfg.name.clone()));
                resp_fields.insert(
                    "finish_reason".into(),
                    Value::String(result.finish_reason.clone()),
                );
                resp_fields.insert(
                    "tool_calls".into(),
                    Value::from(result.tool_calls.len() as u64),
                );
                if let Some(u) = result.usage
                    && let Ok(val) = serde_json::to_value(u.normalized_5())
                {
                    resp_fields.insert("usage".into(), val);
                }
                tracer.emit_event("llm_response", resp_fields);
                return Ok(result);
            }
            Err(fail) => {
                if let Some(code) = fail.interrupted {
                    return Err(ProviderError::Interrupted(code));
                }

                if let Some(ref u) = fail.usage {
                    accumulated_failed_usage.add_assign(u);
                    saw_failed_usage = true;
                }

                // Section 9.5: Persist rich `provider_error` trace on every failure/retry.
                let mut err_fields = Map::new();
                err_fields.insert("provider".into(), Value::String(provider_cfg.name.clone()));
                err_fields.insert("code".into(), Value::String(fail.code.clone()));
                err_fields.insert("message".into(), Value::String(fail.message.clone()));
                err_fields.insert("attempt".into(), Value::from(attempt as u64));
                err_fields.insert("retryable".into(), Value::Bool(fail.retryable));
                err_fields.insert("elapsed_ms".into(), Value::from(fail.elapsed_ms));
                err_fields.insert(
                    "provider_stderr".into(),
                    Value::String(fail.provider_stderr.clone()),
                );
                if !fail.partial_text.is_empty() {
                    err_fields.insert(
                        "partial_text".into(),
                        Value::String(fail.partial_text.clone()),
                    );
                }
                if !fail.partial_reasoning.is_empty() {
                    err_fields.insert(
                        "partial_reasoning".into(),
                        Value::String(fail.partial_reasoning.clone()),
                    );
                }
                if let Some(ts) = fail.last_delta_ts {
                    err_fields.insert("last_delta_ts".into(), Value::String(ts));
                }
                if let Some(u) = fail.usage
                    && let Ok(val) = serde_json::to_value(u.normalized_5())
                {
                    err_fields.insert("usage".into(), val);
                }
                tracer.emit_event("provider_error", err_fields);

                if fail.timed_out {
                    return Err(ProviderError::Timeout {
                        provider: provider_cfg.name.clone(),
                        timeout_sec: provider_cfg.timeout_sec,
                    });
                }

                if fail.output_truncated_empty {
                    return Err(ProviderError::OutputTruncatedEmpty {
                        provider: provider_cfg.name.clone(),
                        message: fail.message,
                        usage: saw_failed_usage.then_some(accumulated_failed_usage),
                    });
                }

                if fail.retryable && attempt < max_attempts {
                    let delay = RETRY_DELAYS_SEC[attempt - 1];
                    if stream_to_stderr && stderr_mode == StderrMode::Auto {
                        stderr_tracker.writeln_line(&format!(
                            "[agent] provider `{}` error ({}) — retrying in {}s (attempt {}/{})",
                            provider_cfg.name,
                            fail.code,
                            delay,
                            attempt,
                            RETRY_DELAYS_SEC.len()
                        ));
                    }
                    if posix::sleep_or_signal(Duration::from_secs(delay), signals.wakeup_fd()) {
                        signals.drain_wakeup();
                        if let Some(code) = signals.termination_exit_code() {
                            return Err(ProviderError::Interrupted(code));
                        }
                    }
                    continue;
                }

                let detail = if !fail.provider_stderr.trim().is_empty() {
                    format!("{} | stderr: {}", fail.message, fail.provider_stderr.trim())
                } else {
                    fail.message
                };
                return Err(ProviderError::Failed {
                    provider: provider_cfg.name.clone(),
                    message: detail,
                    usage: saw_failed_usage.then_some(accumulated_failed_usage),
                });
            }
        }
    }

    Err(ProviderError::Failed {
        provider: provider_cfg.name.clone(),
        message: "provider retry attempts exhausted".into(),
        usage: saw_failed_usage.then_some(accumulated_failed_usage),
    })
}

/// Shared event accumulator used across both `coprocess` and `oneshot` turns.
struct TurnAccumulator {
    text: String,
    reasoning: String,
    tool_calls: Vec<ToolCall>,
    seen_tool_call_ids: HashSet<String>,
    usage: Option<TokenUsage>,
    provider_opaque: Option<Value>,
    finish_reason: Option<String>,
    last_error: Option<(String, String, bool)>,
    last_delta_ts: Option<String>,
    in_dim_ansi: bool,
    is_tty_stderr: bool,
    render_stderr: bool,
}

impl TurnAccumulator {
    fn new(stderr_mode: StderrMode, stream_to_stderr: bool) -> Self {
        Self {
            text: String::new(),
            reasoning: String::new(),
            tool_calls: Vec::new(),
            seen_tool_call_ids: HashSet::new(),
            usage: None,
            provider_opaque: None,
            finish_reason: None,
            last_error: None,
            last_delta_ts: None,
            in_dim_ansi: false,
            is_tty_stderr: posix::isatty(2),
            render_stderr: stream_to_stderr && stderr_mode == StderrMode::Auto,
        }
    }

    /// Process one raw JSONL line from Provider `stdout`.
    /// Returns `true` when a terminal frame (`done` or `error`) is observed.
    fn consume_line(
        &mut self,
        raw_line: &str,
        stderr_tracker: &StderrTracker,
        tracer: &mut Tracer,
    ) -> bool {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            return false;
        }
        if tracer.is_enabled() {
            let mut wf = Map::new();
            wf.insert("line".into(), Value::String(trimmed.to_owned()));
            tracer.emit_wire("wire_provider_frame", wf);
        }
        let Ok(event) = serde_json::from_str::<ProviderEvent>(trimmed) else {
            return false;
        };
        match event {
            ProviderEvent::ReasoningDelta { text: delta } => {
                self.reasoning.push_str(&delta);
                self.last_delta_ts = Some(posix::timestamp_rfc3339_utc());
                tracer.emit_delta("reasoning_delta", &delta);
                self.write_stderr_delta(&delta, stderr_tracker);
                false
            }
            ProviderEvent::TextDelta { text: delta } => {
                self.text.push_str(&delta);
                self.last_delta_ts = Some(posix::timestamp_rfc3339_utc());
                tracer.emit_delta("text_delta", &delta);
                self.write_stderr_delta(&delta, stderr_tracker);
                false
            }
            ProviderEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                if !id.is_empty() && self.seen_tool_call_ids.insert(id.clone()) {
                    self.tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                }
                false
            }
            ProviderEvent::Usage {
                input_tokens,
                output_tokens,
                cached_tokens,
                cache_write_tokens,
                reasoning_tokens,
            } => {
                self.usage = Some(TokenUsage {
                    input_tokens,
                    output_tokens,
                    cached_tokens,
                    cache_write_tokens,
                    reasoning_tokens,
                });
                false
            }
            ProviderEvent::Opaque { data } => {
                self.provider_opaque = Some(data);
                false
            }
            ProviderEvent::Error {
                code,
                message,
                retryable,
            } => {
                self.last_error = Some((code, message, retryable));
                true
            }
            ProviderEvent::Done { finish_reason } => {
                self.finish_reason = Some(finish_reason);
                true
            }
        }
    }

    fn write_stderr_delta(&mut self, delta: &str, stderr_tracker: &StderrTracker) {
        if !self.render_stderr || delta.is_empty() {
            return;
        }
        let mut err = io::stderr().lock();
        if self.is_tty_stderr && !self.in_dim_ansi {
            let _ = err.write_all(b"\x1b[2m");
            self.in_dim_ansi = true;
        }
        let _ = err.write_all(delta.as_bytes());
        let _ = err.flush();
        stderr_tracker.note_bytes(delta.as_bytes());
    }

    /// Close ANSI Dim styling cleanly before any trailing newline (v1.1 §4.1).
    fn finish_stderr_stream(&mut self, stderr_tracker: &StderrTracker) {
        if self.in_dim_ansi {
            let mut err = io::stderr().lock();
            let _ = err.write_all(b"\x1b[0m");
            let _ = err.flush();
            self.in_dim_ansi = false;
        }
        if self.render_stderr {
            stderr_tracker.ensure_newline();
        }
    }

    fn finalize_ok(
        self,
        started: Instant,
        provider_stderr: String,
    ) -> Result<ProviderTurnResult, Box<SingleAttemptFailure>> {
        let finish_reason = self.finish_reason.unwrap_or_else(|| "stop".into());
        if self.text.is_empty() && self.tool_calls.is_empty() {
            let is_length = matches!(finish_reason.as_str(), "length" | "max_tokens");
            let (code, message, retryable) = if is_length {
                (
                    "output_truncated_empty".into(),
                    "model exhausted output token limit without producing text or tool calls (increase --max-output-tokens or lower reasoning_effort)".into(),
                    false,
                )
            } else {
                (
                    "empty_response".into(),
                    format!(
                        "provider returned empty response (finish_reason={finish_reason}) with neither text nor tool calls"
                    ),
                    true,
                )
            };
            return Err(Box::new(SingleAttemptFailure {
                code,
                message,
                retryable,
                timed_out: false,
                output_truncated_empty: is_length,
                interrupted: None,
                provider_stderr,
                partial_text: self.text,
                partial_reasoning: self.reasoning,
                last_delta_ts: self.last_delta_ts,
                usage: self.usage,
                elapsed_ms: started.elapsed().as_millis() as u64,
            }));
        }

        Ok(ProviderTurnResult {
            text: self.text,
            reasoning: self.reasoning,
            tool_calls: self.tool_calls,
            usage: self.usage,
            provider_opaque: self.provider_opaque,
            finish_reason,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn run_coprocess_attempt(
    pool: &mut ProviderPool,
    bin_path: &Path,
    provider_cfg: &ProviderConfig,
    model: &str,
    api_key: &str,
    stream: &IrStream,
    cwd: &Path,
    stderr_mode: StderrMode,
    stream_to_stderr: bool,
    signals: &SignalState,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> Result<ProviderTurnResult, Box<SingleAttemptFailure>> {
    let started = Instant::now();
    let pool_key = (
        provider_cfg.name.clone(),
        bin_path.to_path_buf(),
        provider_cfg.base_url.clone(),
    );

    let req_id = pool.alloc_request_id();
    let envelope =
        ProviderRequestEnvelope::from_stream(stream, req_id, model, provider_cfg.timeout_sec);
    let mut req_line = serde_json::to_string(&envelope).map_err(|e| {
        Box::new(SingleAttemptFailure {
            code: "serialize_error".into(),
            message: format!("failed to serialize coprocess request: {e}"),
            retryable: false,
            timed_out: false,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr: String::new(),
            partial_text: String::new(),
            partial_reasoning: String::new(),
            last_delta_ts: None,
            usage: None,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    })?;
    req_line.push('\n');

    // Obtain existing live coprocess or spawn a fresh one; if writing fails on a stale pipe,
    // respawn cleanly once (v1.1 §9.2 crash recovery).
    let spawn_err = |err: io::Error| {
        Box::new(SingleAttemptFailure {
            code: "spawn_error".into(),
            message: format!("failed to execute `{}`: {err}", bin_path.display()),
            retryable: false,
            timed_out: false,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr: String::new(),
            partial_text: String::new(),
            partial_reasoning: String::new(),
            last_delta_ts: None,
            usage: None,
            elapsed_ms: started.elapsed().as_millis() as u64,
        })
    };
    let mut handle = match pool.coprocesses.remove(&pool_key) {
        Some(mut h) => {
            if h.is_alive() {
                h.clear_stderr();
                h
            } else {
                h.terminate_now();
                CoprocessHandle::spawn(bin_path, provider_cfg, model, api_key, cwd)
                    .map_err(spawn_err)?
            }
        }
        None => CoprocessHandle::spawn(bin_path, provider_cfg, model, api_key, cwd)
            .map_err(spawn_err)?,
    };

    let write_ok = handle.stdin.as_mut().is_some_and(|s| {
        s.write_all(req_line.as_bytes())
            .and_then(|_| s.flush())
            .is_ok()
    });
    if !write_ok {
        handle.terminate_now();
        handle =
            CoprocessHandle::spawn(bin_path, provider_cfg, model, api_key, cwd).map_err(|err| {
                Box::new(SingleAttemptFailure {
                    code: "spawn_error".into(),
                    message: format!("failed to respawn `{}`: {err}", bin_path.display()),
                    retryable: false,
                    timed_out: false,
                    output_truncated_empty: false,
                    interrupted: None,
                    provider_stderr: String::new(),
                    partial_text: String::new(),
                    partial_reasoning: String::new(),
                    last_delta_ts: None,
                    usage: None,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                })
            })?;
        let retry_write = handle
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::other("coprocess stdin missing"))
            .and_then(|s| s.write_all(req_line.as_bytes()).and_then(|_| s.flush()));
        if let Err(err) = retry_write {
            let stderr = handle.take_stderr_snapshot();
            handle.terminate_now();
            return Err(Box::new(SingleAttemptFailure {
                code: "stdin_write_error".into(),
                message: format!("failed to write coprocess request: {err}"),
                retryable: true,
                timed_out: false,
                output_truncated_empty: false,
                interrupted: None,
                provider_stderr: stderr,
                partial_text: String::new(),
                partial_reasoning: String::new(),
                last_delta_ts: None,
                usage: None,
                elapsed_ms: started.elapsed().as_millis() as u64,
            }));
        }
    }

    signals.set_active_child_pgid(handle.pgid);

    let timeout = Duration::from_secs(provider_cfg.timeout_sec.max(1));
    let deadline = started + timeout;
    let mut acc = TurnAccumulator::new(stderr_mode, stream_to_stderr);
    let mut timed_out = false;
    let mut interrupted: Option<ExitCode> = None;
    let mut unexpected_eof = false;
    let mut read_io_err: Option<String> = None;

    loop {
        if let Some(code) = signals.termination_exit_code() {
            interrupted = Some(code);
            break;
        }
        match handle.stdout.read_line_until(deadline, signals) {
            Ok(ReadLineOutcome::Line(line)) => {
                if acc.consume_line(&line, stderr_tracker, tracer) {
                    break;
                }
            }
            Ok(ReadLineOutcome::Eof) => {
                unexpected_eof = true;
                break;
            }
            Ok(ReadLineOutcome::TimedOut) => {
                timed_out = true;
                break;
            }
            Ok(ReadLineOutcome::SignalWakeup) => {
                if let Some(code) = signals.termination_exit_code() {
                    interrupted = Some(code);
                    break;
                }
            }
            Err(err) => {
                read_io_err = Some(err.to_string());
                break;
            }
        }
    }

    acc.finish_stderr_stream(stderr_tracker);
    signals.clear_active_child_pgid();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if interrupted.is_some() {
        handle.terminate_now();
        return Err(Box::new(SingleAttemptFailure {
            code: "interrupted".into(),
            message: "interrupted by signal".into(),
            retryable: false,
            timed_out: false,
            output_truncated_empty: false,
            interrupted,
            provider_stderr: String::new(),
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    if timed_out {
        let provider_stderr = handle.take_stderr_snapshot();
        handle.terminate_now();
        return Err(Box::new(SingleAttemptFailure {
            code: "timeout".into(),
            message: format!("provider timed out after {}s", provider_cfg.timeout_sec),
            retryable: false,
            timed_out: true,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    let provider_stderr = handle.take_stderr_snapshot();

    // If transport broke or coprocess exited unexpectedly, evict & terminate it cleanly.
    if unexpected_eof || read_io_err.is_some() || !handle.is_alive() {
        let child_exit = handle
            .child
            .try_wait()
            .ok()
            .flatten()
            .and_then(|s| s.code());
        handle.terminate_now();
        if let Some((code, message, retryable)) = acc.last_error {
            let is_timeout =
                code == "timeout" || child_exit == Some(ExitCode::TimeoutOrBudget.as_i32());
            return Err(Box::new(SingleAttemptFailure {
                code,
                message,
                retryable: retryable && !is_timeout,
                timed_out: is_timeout,
                output_truncated_empty: false,
                interrupted: None,
                provider_stderr,
                partial_text: acc.text,
                partial_reasoning: acc.reasoning,
                last_delta_ts: acc.last_delta_ts,
                usage: acc.usage,
                elapsed_ms,
            }));
        }
        if acc.finish_reason.is_some() {
            return acc.finalize_ok(started, provider_stderr);
        }
        let (code, message) = if let Some(err) = read_io_err {
            ("stdout_read_error".into(), err)
        } else if let Some(ec) = child_exit
            && ec != 0
        {
            (
                "provider_exit_nonzero".into(),
                format!("provider coprocess exited with status {ec}"),
            )
        } else {
            (
                "missing_done_event".into(),
                "provider coprocess closed stdout without emitting `done` or `error`".into(),
            )
        };
        return Err(Box::new(SingleAttemptFailure {
            code,
            message,
            retryable: true,
            timed_out: false,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    // Coprocess is healthy and idle awaiting the next request — return it to the pool!
    pool.coprocesses.insert(pool_key, handle);

    if let Some((code, message, retryable)) = acc.last_error {
        let is_timeout = code == "timeout";
        return Err(Box::new(SingleAttemptFailure {
            code,
            message,
            retryable,
            timed_out: is_timeout && !retryable,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    acc.finalize_ok(started, provider_stderr)
}

#[allow(clippy::too_many_arguments)]
fn run_oneshot_attempt(
    bin_path: &Path,
    provider_cfg: &ProviderConfig,
    model: &str,
    api_key: &str,
    ir_jsonl: &str,
    cwd: &Path,
    stderr_mode: StderrMode,
    stream_to_stderr: bool,
    signals: &SignalState,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> Result<ProviderTurnResult, Box<SingleAttemptFailure>> {
    let started = Instant::now();
    let parent_pid = unsafe { libc::getpid() };

    let mut cmd = Command::new(bin_path);
    cmd.current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("AGENT_PROVIDER_BASE_URL", &provider_cfg.base_url)
        .env("AGENT_PROVIDER_API_KEY", api_key)
        .env(
            "AGENT_PROVIDER_TIMEOUT_SEC",
            provider_cfg.timeout_sec.to_string(),
        )
        .env("AGENT_MODEL", model)
        .env("AGENT_PROVIDER_MODE", ProviderMode::Oneshot.as_str());

    // SAFETY: `pre_exec` invokes only async-signal-safe syscalls to isolate PGID and set PDEATHSIG.
    unsafe {
        cmd.pre_exec(move || {
            if libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            #[cfg(target_os = "linux")]
            {
                crate::platform::linux::set_pdeathsig_sigkill(parent_pid)?;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = parent_pid;
            Ok(())
        });
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            return Err(Box::new(SingleAttemptFailure {
                code: "spawn_error".into(),
                message: format!("failed to execute `{}`: {err}", bin_path.display()),
                retryable: false,
                timed_out: false,
                output_truncated_empty: false,
                interrupted: None,
                provider_stderr: String::new(),
                partial_text: String::new(),
                partial_reasoning: String::new(),
                last_delta_ts: None,
                usage: None,
                elapsed_ms: started.elapsed().as_millis() as u64,
            }));
        }
    };

    let child_pgid = child.id() as libc::pid_t;
    signals.set_active_child_pgid(child_pgid);

    if let Some(mut stdin) = child.stdin.take() {
        let payload = ir_jsonl.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let Some(stdout_pipe) = child.stdout.take() else {
        let _ = signals.terminate_active_child_and_wait(&mut child);
        signals.clear_active_child_pgid();
        return Err(Box::new(SingleAttemptFailure {
            code: "stdout_missing".into(),
            message: "provider stdout pipe missing".into(),
            retryable: false,
            timed_out: false,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr: String::new(),
            partial_text: String::new(),
            partial_reasoning: String::new(),
            last_delta_ts: None,
            usage: None,
            elapsed_ms: started.elapsed().as_millis() as u64,
        }));
    };
    let stderr_pipe = child.stderr.take();
    let stderr_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut p) = stderr_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let mut reader = BufferedStdoutReader::new(stdout_pipe);
    let timeout = Duration::from_secs(provider_cfg.timeout_sec.max(1));
    let deadline = started + timeout;
    let mut acc = TurnAccumulator::new(stderr_mode, stream_to_stderr);
    let mut timed_out = false;
    let mut interrupted: Option<ExitCode> = None;
    let mut status_override: Option<io::Result<std::process::ExitStatus>> = None;

    loop {
        if let Some(code) = signals.termination_exit_code() {
            interrupted = Some(code);
            status_override = Some(signals.terminate_active_child_and_wait(&mut child));
            break;
        }
        match reader.read_line_until(deadline, signals) {
            Ok(ReadLineOutcome::Line(line)) => {
                acc.consume_line(&line, stderr_tracker, tracer);
            }
            Ok(ReadLineOutcome::Eof) => break,
            Ok(ReadLineOutcome::TimedOut) => {
                timed_out = true;
                status_override = Some(signals.terminate_active_child_and_wait(&mut child));
                break;
            }
            Ok(ReadLineOutcome::SignalWakeup) => {
                if let Some(code) = signals.termination_exit_code() {
                    interrupted = Some(code);
                    status_override = Some(signals.terminate_active_child_and_wait(&mut child));
                    break;
                }
            }
            Err(err) => {
                acc.last_error = Some(("stdout_read_error".into(), err.to_string(), true));
                break;
            }
        }
    }

    acc.finish_stderr_stream(stderr_tracker);

    let status = match status_override {
        Some(st) => st,
        None => loop {
            if let Some(code) = signals.termination_exit_code() {
                interrupted = Some(code);
                break signals.terminate_active_child_and_wait(&mut child);
            }
            match platform::wait_child_until(&mut child, Some(deadline), Some(signals.wakeup_fd()))
            {
                Ok(ChildWaitOutcome::Exited(st)) => break Ok(st),
                Ok(ChildWaitOutcome::SignalWakeup) => {
                    signals.drain_wakeup();
                    continue;
                }
                Ok(ChildWaitOutcome::TimedOut) => {
                    timed_out = true;
                    break signals.terminate_active_child_and_wait(&mut child);
                }
                Err(err) => break Err(err),
            }
        },
    };
    signals.clear_active_child_pgid();
    let elapsed_ms = started.elapsed().as_millis() as u64;

    if interrupted.is_some() {
        return Err(Box::new(SingleAttemptFailure {
            code: "interrupted".into(),
            message: "interrupted by signal".into(),
            retryable: false,
            timed_out: false,
            output_truncated_empty: false,
            interrupted,
            provider_stderr: String::new(),
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    if timed_out {
        return Err(Box::new(SingleAttemptFailure {
            code: "timeout".into(),
            message: format!("provider timed out after {}s", provider_cfg.timeout_sec),
            retryable: false,
            timed_out: true,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr: String::new(),
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    let provider_stderr = stderr_handle.join().unwrap_or_default();
    let child_exit_code = status.ok().and_then(|s| s.code()).unwrap_or(1);
    if child_exit_code == ExitCode::TimeoutOrBudget.as_i32() {
        return Err(Box::new(SingleAttemptFailure {
            code: "timeout".into(),
            message: format!("provider timed out after {}s", provider_cfg.timeout_sec),
            retryable: false,
            timed_out: true,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    if child_exit_code != 0 || acc.finish_reason.is_none() {
        let (code, message, retryable) = acc.last_error.unwrap_or_else(|| {
            if child_exit_code != 0 {
                (
                    "provider_exit_nonzero".into(),
                    format!("provider exited with status {child_exit_code}"),
                    false,
                )
            } else {
                (
                    "missing_done_event".into(),
                    "provider closed stdout without emitting a `done` event".into(),
                    true,
                )
            }
        });
        return Err(Box::new(SingleAttemptFailure {
            code,
            message,
            retryable,
            timed_out: false,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: acc.text,
            partial_reasoning: acc.reasoning,
            last_delta_ts: acc.last_delta_ts,
            usage: acc.usage,
            elapsed_ms,
        }));
    }

    acc.finalize_ok(started, provider_stderr)
}
