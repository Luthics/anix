//! External Provider adapter runner (`git-remote-*` pattern), streaming JSONL event parser,
//! and main-process exponential backoff retry (1s / 2s / 4s, max 3 retries) (Section 9.2).

use std::collections::HashSet;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{IrStream, StderrMode, ToolCall};
use os_agent_proto::provider::{ProviderEvent, TokenUsage};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::config::{AgentPaths, ProviderConfig};
use crate::platform::posix;
use crate::signal::SignalState;
use crate::trace::Tracer;
use crate::trust::{self, TrustError};

const RETRY_DELAYS_SEC: &[u64] = &[1, 2, 4];

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
            Self::Trust(_) => ExitCode::PermissionDenied,
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

/// Invoke the configured Provider adapter with Canonical IR v1 JSONL on `stdin` and
/// perform deterministic exponential backoff retries (`1s / 2s / 4s`) on retryable errors.
#[allow(clippy::too_many_arguments)]
pub fn invoke_provider(
    provider_cfg: &ProviderConfig,
    model: &str,
    stream: &IrStream,
    paths: &AgentPaths,
    stderr_mode: StderrMode,
    stream_to_stderr: bool,
    signals: &SignalState,
    tracer: &mut Tracer,
) -> Result<ProviderTurnResult, ProviderError> {
    let base_dir = paths.project_agent_dir.as_deref().and_then(|d| d.parent());
    let bin_path = paths.resolve_component_bin(&provider_cfg.bin, base_dir);

    if provider_cfg.bin_from_project {
        trust::verify_trusted(&bin_path, paths)?;
    }

    let ir_jsonl = stream.to_jsonl().map_err(|e| ProviderError::Failed {
        provider: provider_cfg.name.clone(),
        message: format!("failed to serialize Canonical IR: {e}"),
        usage: None,
    })?;

    if tracer.is_enabled() {
        let mut wire = Map::new();
        wire.insert("provider".into(), Value::String(provider_cfg.name.clone()));
        wire.insert("model".into(), Value::String(model.to_owned()));
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
        req_fields.insert("attempt".into(), Value::from(attempt as u64));
        req_fields.insert("messages".into(), Value::from(stream.messages.len() as u64));
        tracer.emit_event("llm_request_start", req_fields);

        match run_single_attempt(
            &bin_path,
            provider_cfg,
            model,
            &api_key,
            &ir_jsonl,
            &paths.cwd,
            stderr_mode,
            stream_to_stderr,
            signals,
            tracer,
        ) {
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
                        eprintln!(
                            "\n[agent] provider `{}` error ({}) — retrying in {}s (attempt {}/{})",
                            provider_cfg.name,
                            fail.code,
                            delay,
                            attempt,
                            RETRY_DELAYS_SEC.len()
                        );
                    }
                    if sleep_interruptible(Duration::from_secs(delay), signals)
                        && let Some(code) = signals.termination_exit_code()
                    {
                        return Err(ProviderError::Interrupted(code));
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

#[allow(clippy::too_many_arguments)]
fn run_single_attempt(
    bin_path: &std::path::Path,
    provider_cfg: &ProviderConfig,
    model: &str,
    api_key: &str,
    ir_jsonl: &str,
    cwd: &std::path::Path,
    stderr_mode: StderrMode,
    stream_to_stderr: bool,
    signals: &SignalState,
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
        .env("AGENT_MODEL", model);

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

    // Write Canonical IR JSONL to provider stdin and close write end (EOF).
    if let Some(mut stdin) = child.stdin.take() {
        let payload = ir_jsonl.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        });
    }

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stderr_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(mut p) = stderr_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let (tx, rx) = mpsc::channel::<io::Result<Option<String>>>();
    let stdout_handle = thread::spawn(move || {
        let Some(pipe) = stdout_pipe else {
            let _ = tx.send(Ok(None));
            return;
        };
        let mut reader = BufReader::new(pipe);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(Ok(None));
                    break;
                }
                Ok(_) => {
                    if tx.send(Ok(Some(line))).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(err));
                    break;
                }
            }
        }
    });

    let timeout = Duration::from_secs(provider_cfg.timeout_sec.max(1));
    let deadline = started + timeout;
    let is_tty_stderr = posix::isatty(2);
    let render_stderr = stream_to_stderr && stderr_mode == StderrMode::Auto;

    let mut text = String::new();
    let mut reasoning = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    let mut seen_tool_call_ids: HashSet<String> = HashSet::new();
    let mut usage: Option<TokenUsage> = None;
    let mut provider_opaque: Option<Value> = None;
    let mut finish_reason: Option<String> = None;
    let mut last_error: Option<(String, String, bool)> = None;
    let mut last_delta_ts: Option<String> = None;
    let mut printed_any_stderr = false;
    let mut in_dim_ansi = false;
    let mut timed_out = false;
    let mut interrupted: Option<ExitCode> = None;

    let mut status_override: Option<io::Result<std::process::ExitStatus>> = None;

    loop {
        if let Some(code) = signals.termination_exit_code() {
            interrupted = Some(code);
            status_override = Some(signals.terminate_active_child_and_wait(&mut child));
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            status_override = Some(signals.terminate_active_child_and_wait(&mut child));
            break;
        }

        match rx.recv_timeout(Duration::from_millis(40)) {
            Ok(Ok(Some(raw_line))) => {
                let trimmed = raw_line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if tracer.is_enabled() {
                    let mut wf = Map::new();
                    wf.insert("line".into(), Value::String(trimmed.to_owned()));
                    tracer.emit_wire("wire_provider_frame", wf);
                }
                let Ok(event) = serde_json::from_str::<ProviderEvent>(trimmed) else {
                    continue;
                };
                match event {
                    ProviderEvent::ReasoningDelta { text: delta } => {
                        reasoning.push_str(&delta);
                        last_delta_ts = Some(posix::timestamp_rfc3339_utc());
                        tracer.emit_delta("reasoning_delta", &delta);
                        if render_stderr {
                            let mut err = io::stderr().lock();
                            if is_tty_stderr && !in_dim_ansi {
                                let _ = err.write_all(b"\x1b[2m");
                                in_dim_ansi = true;
                            }
                            let _ = err.write_all(delta.as_bytes());
                            let _ = err.flush();
                            printed_any_stderr = true;
                        }
                    }
                    ProviderEvent::TextDelta { text: delta } => {
                        text.push_str(&delta);
                        last_delta_ts = Some(posix::timestamp_rfc3339_utc());
                        tracer.emit_delta("text_delta", &delta);
                        if render_stderr {
                            let mut err = io::stderr().lock();
                            if is_tty_stderr && !in_dim_ansi {
                                let _ = err.write_all(b"\x1b[2m");
                                in_dim_ansi = true;
                            }
                            let _ = err.write_all(delta.as_bytes());
                            let _ = err.flush();
                            printed_any_stderr = true;
                        }
                    }
                    ProviderEvent::ToolCall {
                        id,
                        name,
                        arguments,
                    } => {
                        if !id.is_empty() && seen_tool_call_ids.insert(id.clone()) {
                            tool_calls.push(ToolCall {
                                id,
                                name,
                                arguments,
                            });
                        }
                    }
                    ProviderEvent::Usage {
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        cache_write_tokens,
                        reasoning_tokens,
                    } => {
                        usage = Some(TokenUsage {
                            input_tokens,
                            output_tokens,
                            cached_tokens,
                            cache_write_tokens,
                            reasoning_tokens,
                        });
                    }
                    ProviderEvent::Opaque { data } => {
                        provider_opaque = Some(data);
                    }
                    ProviderEvent::Error {
                        code,
                        message,
                        retryable,
                    } => {
                        last_error = Some((code, message, retryable));
                    }
                    ProviderEvent::Done { finish_reason: fr } => {
                        finish_reason = Some(fr);
                    }
                }
            }
            Ok(Ok(None)) => break,
            Ok(Err(err)) => {
                last_error = Some(("stdout_read_error".into(), err.to_string(), true));
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    if in_dim_ansi {
        let mut err = io::stderr().lock();
        let _ = err.write_all(b"\x1b[0m");
        let _ = err.flush();
    }
    if printed_any_stderr && !text.ends_with('\n') && !reasoning.ends_with('\n') {
        eprintln!();
    }

    let status = match status_override {
        Some(st) => st,
        None => child.wait(),
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
            partial_text: text,
            partial_reasoning: reasoning,
            last_delta_ts,
            usage,
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
            partial_text: text,
            partial_reasoning: reasoning,
            last_delta_ts,
            usage,
            elapsed_ms,
        }));
    }

    let _ = stdout_handle.join();
    let provider_stderr = stderr_handle.join().unwrap_or_default();

    let child_exit_code = status.ok().and_then(|s| s.code()).unwrap_or(1);
    if timed_out || child_exit_code == ExitCode::TimeoutOrBudget.as_i32() {
        return Err(Box::new(SingleAttemptFailure {
            code: "timeout".into(),
            message: format!("provider timed out after {}s", provider_cfg.timeout_sec),
            retryable: false,
            timed_out: true,
            output_truncated_empty: false,
            interrupted: None,
            provider_stderr,
            partial_text: text,
            partial_reasoning: reasoning,
            last_delta_ts,
            usage,
            elapsed_ms,
        }));
    }

    // Section 9.2: Non-zero exit OR closing stdout without `done` is an error.
    if child_exit_code != 0 || finish_reason.is_none() {
        let (code, message, retryable) = last_error.unwrap_or_else(|| {
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
            partial_text: text,
            partial_reasoning: reasoning,
            last_delta_ts,
            usage,
            elapsed_ms,
        }));
    }

    let finish_reason = finish_reason.unwrap_or_else(|| "stop".into());

    // Section 9.1: An assistant turn must NEVER have both empty `text` and empty `tool_calls`.
    // Reject before writing to WAL so a reasoning-truncated or empty upstream reply can never
    // poison a persistent `-c` session:
    // - `length` / `max_tokens`: deterministic budget exhaustion -> fail fast (exit 124, no retry).
    // - Other finish reasons: transient empty completion -> retryable with exponential backoff.
    if text.is_empty() && tool_calls.is_empty() {
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
            partial_text: text,
            partial_reasoning: reasoning,
            last_delta_ts,
            usage,
            elapsed_ms,
        }));
    }

    Ok(ProviderTurnResult {
        text,
        reasoning,
        tool_calls,
        usage,
        provider_opaque,
        finish_reason,
    })
}

fn sleep_interruptible(duration: Duration, signals: &SignalState) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if signals.termination_exit_code().is_some() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}
