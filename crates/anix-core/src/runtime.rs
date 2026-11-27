//! Core inference loop state machine, shell execution, `$ANIX_CTL` (`fd 6`) frame
//! application (`media`, `compact`, `fold`, `hold`, `release`), unified Maildir `--inbox`
//! event loop with debounce batching, crash dedup, `Anix-Reply-To` sync replies,
//! `Anix-Control: compact`, and `--notify` exit callbacks (RFC §§4–8).

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anix_proto::IR_VERSION;
use anix_proto::ctl::CtlFrame;
use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{
    ALLOWED_MEDIA_MIMES, HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, MediaAttachment,
    OutputMode, StderrMode,
};
use anix_proto::provider::TokenUsage;
use anix_proto::trace::ContextWatermark;
use anix_proto::wal::{WalConfigSnapshot, WalEvent};
use serde_json::{Map, Value};

use crate::compact::{self, CompactError};
use crate::config::ResolvedConfig;
use crate::fd::{self, StderrTracker};
use crate::frontmatter::AgentScript;
use crate::hooks;
use crate::maildir::{self, InboxMessage, InboxQueue, InboxWaitOutcome};
use crate::platform::{self, PreparedSandbox, posix};
use crate::provider::{self, ProviderError, ProviderPool, TextStreamMode};
use crate::signal::{self, SignalState};
use crate::trace::Tracer;
use crate::wal::{self, ActiveEntry, WalStore};

/// Structured termination reason emitted on `cycle_end` Trace events (RFC §8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CycleEndReason {
    Completed,
    MaxTurns,
    MaxTokens,
    GateBlocked,
    ProviderError,
    Error,
    Interrupted,
}

impl CycleEndReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::MaxTurns => "max_turns",
            Self::MaxTokens => "max_tokens",
            Self::GateBlocked => "gate_blocked",
            Self::ProviderError => "provider_error",
            Self::Error => "error",
            Self::Interrupted => "interrupted",
        }
    }
}

struct CycleOutcome {
    exit_code: ExitCode,
    reason: CycleEndReason,
    turns: u32,
    delivered_output: String,
}

#[derive(Debug, Clone)]
struct ActiveHold {
    id: String,
    deadline: Instant,
    label: Option<String>,
}

/// Entry point for executing a resolved `*.agent` session (one-shot or `--inbox`).
pub fn execute(cfg: &ResolvedConfig, script: &AgentScript, script_args: &[String]) -> ExitCode {
    posix::ignore_sigpipe();
    let trace_file = match cfg.trace_fd {
        Some(fd) => match Tracer::take_configured_fd(fd) {
            Ok(f) => Some(f),
            Err(msg) => {
                eprintln!("anix: {msg}");
                deliver_notify_if_configured(cfg, ExitCode::UsageError, &msg);
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let inherited_extra_fds = fd::drain_inherited_extra_fds(cfg.trace_fd);

    let signals = match SignalState::install() {
        Ok(s) => s,
        Err(err) => {
            let msg = format!("failed to install signal handlers: {err}");
            eprintln!("anix: {msg}");
            deliver_notify_if_configured(cfg, ExitCode::RuntimeError, &msg);
            return ExitCode::RuntimeError;
        }
    };

    let stderr_tracker = StderrTracker::new();
    let mut provider_pool = ProviderPool::new();

    let _ = posix::ensure_dir_mode(&cfg.paths.runtime_dir, 0o700);

    let sandbox: Option<PreparedSandbox> = if cfg.shell_enabled() {
        match platform::prepare_sandbox(cfg.sandbox, &cfg.paths.cwd, &cfg.paths.runtime_dir) {
            Ok(sb) => sb,
            Err(msg) => {
                stderr_tracker.writeln_line(&format!("anix: {msg}"));
                deliver_notify_if_configured(cfg, ExitCode::RuntimeError, &msg);
                return ExitCode::RuntimeError;
            }
        }
    } else {
        None
    };

    let mut tracer = Tracer::init(&cfg.name, trace_file, &cfg.paths.journal_socket_path());

    // Open WAL context store (`-c <file>`) with optional `--lock-wait[=<SEC>]` (RFC §5.2).
    let mut wal = match cfg.context {
        Some(ref path) => match WalStore::open_with_lock_wait(path, cfg.lock_wait, &signals) {
            Ok(store) => store,
            Err(err) => {
                let msg = err.to_string();
                stderr_tracker.writeln_line(&format!("anix: {msg}"));
                let code = err.exit_code();
                deliver_notify_if_configured(cfg, code, &msg);
                return code;
            }
        },
        None => WalStore::stateless(),
    };

    let replayed = match wal.load_active_window(cfg.leaf) {
        Ok(r) => r,
        Err(err) => {
            let msg = err.to_string();
            stderr_tracker.writeln_line(&format!("anix: {msg}"));
            let code = err.exit_code();
            deliver_notify_if_configured(cfg, code, &msg);
            return code;
        }
    };

    let session_id = if replayed.is_fresh {
        uuid::Uuid::new_v4().to_string()
    } else {
        replayed
            .session_id
            .clone()
            .unwrap_or_else(|| wal::derive_legacy_session_id(wal.path()))
    };

    let mut run_fields = Map::new();
    run_fields.insert("session_id".into(), Value::String(session_id.clone()));
    run_fields.insert("provider".into(), Value::String(cfg.provider.clone()));
    run_fields.insert("model".into(), Value::String(cfg.model.clone()));
    run_fields.insert("shell".into(), Value::String(cfg.shell.clone()));
    run_fields.insert("tool_name".into(), Value::String(cfg.tool_name.clone()));
    run_fields.insert("output".into(), Value::String(cfg.output.as_str().into()));
    run_fields.insert("fresh_session".into(), Value::Bool(replayed.is_fresh));
    run_fields.insert("inbox_mode".into(), Value::Bool(cfg.inbox.is_some()));
    if cfg.context.is_some()
        && let Some(lid) = replayed.head_id
    {
        run_fields.insert("leaf_id".into(), Value::from(lid));
    }
    tracer.emit_event("run_start", run_fields);

    for warn in cfg.startup_warnings(&script.prompt) {
        stderr_tracker.writeln_line(&format!("anix: warning: {warn}"));
        let mut wf = Map::new();
        wf.insert("message".into(), Value::String(warn));
        tracer.emit_event("warning", wf);
    }

    let mut state =
        match SessionState::init(cfg, script, session_id, replayed, &mut wal, &stderr_tracker) {
            Ok(s) => s,
            Err(code) => {
                provider_pool.shutdown();
                finish_session(
                    cfg,
                    code,
                    0,
                    TokenUsage::default(),
                    TokenUsage::default(),
                    0,
                    None,
                    "",
                    true,
                    &stderr_tracker,
                    &mut tracer,
                );
                return code;
            }
        };

    let mut usage_already_printed_on_cycle = false;
    let mut final_output = String::new();

    let exit_code = if let Some(ref inbox_dir) = cfg.inbox {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, true, &signals) {
            Ok(initial) if !initial.is_empty() => {
                if let Err(code) =
                    state.push_user_message(&initial, Vec::new(), &mut wal, &stderr_tracker)
                {
                    provider_pool.shutdown();
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
                        state.reported_leaf_id(cfg),
                        "",
                        true,
                        &stderr_tracker,
                        &mut tracer,
                    );
                    return code;
                }
                let outcome = execute_tracked_cycle(
                    "initial",
                    None,
                    cfg,
                    script,
                    &mut state,
                    &mut wal,
                    sandbox.as_ref(),
                    &signals,
                    &mut provider_pool,
                    &stderr_tracker,
                    &mut tracer,
                    true,
                );
                final_output = outcome.delivered_output;
                usage_already_printed_on_cycle = true;
                if outcome.exit_code != ExitCode::Success {
                    provider_pool.shutdown();
                    finish_session(
                        cfg,
                        outcome.exit_code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
                        state.reported_leaf_id(cfg),
                        &final_output,
                        false,
                        &stderr_tracker,
                        &mut tracer,
                    );
                    return outcome.exit_code;
                }
            }
            Ok(_) => {}
            Err(err) => {
                let code = if let Some(sig_code) = signals.termination_exit_code() {
                    sig_code
                } else {
                    stderr_tracker
                        .writeln_line(&format!("anix: failed reading initial input FDs: {err}"));
                    ExitCode::RuntimeError
                };
                provider_pool.shutdown();
                finish_session(
                    cfg,
                    code,
                    state.turns_executed,
                    state.run_usage,
                    state.session_usage,
                    state.current_context_tokens(),
                    state.reported_leaf_id(cfg),
                    "",
                    true,
                    &stderr_tracker,
                    &mut tracer,
                );
                return code;
            }
        }

        let (code, printed_usage, last_out) = run_inbox_loop(
            inbox_dir,
            cfg,
            script,
            &mut state,
            &mut wal,
            sandbox.as_ref(),
            &signals,
            &mut provider_pool,
            &stderr_tracker,
            &mut tracer,
        );
        usage_already_printed_on_cycle |= printed_usage;
        if !last_out.is_empty() {
            final_output = last_out;
        }
        code
    } else {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, false, &signals) {
            Ok(input) => {
                if !input.is_empty() {
                    if let Err(code) =
                        state.push_user_message(&input, Vec::new(), &mut wal, &stderr_tracker)
                    {
                        provider_pool.shutdown();
                        finish_session(
                            cfg,
                            code,
                            state.turns_executed,
                            state.run_usage,
                            state.session_usage,
                            state.current_context_tokens(),
                            state.reported_leaf_id(cfg),
                            "",
                            true,
                            &stderr_tracker,
                            &mut tracer,
                        );
                        return code;
                    }
                } else if state.entries.len() <= 1
                    && let Err(code) =
                        state.push_user_message("", Vec::new(), &mut wal, &stderr_tracker)
                {
                    provider_pool.shutdown();
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
                        state.reported_leaf_id(cfg),
                        "",
                        true,
                        &stderr_tracker,
                        &mut tracer,
                    );
                    return code;
                }

                let outcome = execute_tracked_cycle(
                    "initial",
                    None,
                    cfg,
                    script,
                    &mut state,
                    &mut wal,
                    sandbox.as_ref(),
                    &signals,
                    &mut provider_pool,
                    &stderr_tracker,
                    &mut tracer,
                    false,
                );
                final_output = outcome.delivered_output;
                outcome.exit_code
            }
            Err(err) => {
                if let Some(sig_code) = signals.termination_exit_code() {
                    sig_code
                } else {
                    stderr_tracker.writeln_line(&format!("anix: failed reading input FDs: {err}"));
                    ExitCode::RuntimeError
                }
            }
        }
    };

    provider_pool.shutdown();

    finish_session(
        cfg,
        exit_code,
        state.turns_executed,
        state.run_usage,
        state.session_usage,
        state.current_context_tokens(),
        state.reported_leaf_id(cfg),
        &final_output,
        !usage_already_printed_on_cycle,
        &stderr_tracker,
        &mut tracer,
    );
    exit_code
}

struct SessionState {
    session_id: String,
    entries: Vec<ActiveEntry>,
    assistant_usages: BTreeMap<u64, TokenUsage>,
    last_checkpoint_turn: u64,
    next_turn_id: u64,
    tree_mode: bool,
    head_id: Option<u64>,
    next_node_id: u64,
    next_cycle_id: u64,
    turns_executed: u32,
    run_usage: TokenUsage,
    session_usage: TokenUsage,
    pending_compact: bool,
    /// Active generic `hold` leases blocking `--idle-timeout` (RFC §4.3.3 & §6.2).
    active_holds: Vec<ActiveHold>,
    /// Maildir `message_id`s committed to WAL on this lineage (for crash dedup).
    committed_message_ids: HashSet<String>,
}

impl SessionState {
    fn init(
        cfg: &ResolvedConfig,
        script: &AgentScript,
        session_id: String,
        replayed: wal::ReplayedSession,
        wal: &mut WalStore,
        stderr_tracker: &StderrTracker,
    ) -> Result<Self, ExitCode> {
        if replayed.is_fresh {
            let seed = IrStream {
                header: make_ir_header(cfg),
                messages: vec![IrMessage::system(0, &script.prompt)],
            };
            let processed = match hooks::run_pre_context(
                seed,
                &cfg.hooks_dir,
                &cfg.paths.cwd,
                cfg.output,
                cfg.shell_enabled(),
                cfg.stderr,
                &cfg.tool_name,
                cfg.inbox.is_some(),
            ) {
                Ok(ir) => ir,
                Err(err) => {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
            };

            let mut next_node_id = replayed.next_node_id;
            let init_node_id = if replayed.tree_mode {
                let id = next_node_id;
                next_node_id = next_node_id.saturating_add(1);
                Some(id)
            } else {
                None
            };
            let mut head_id = init_node_id;

            let ts = posix::timestamp_rfc3339_utc();
            if let Err(err) = wal.append(&WalEvent::SessionInit {
                id: init_node_id,
                ir_version: IR_VERSION,
                ts,
                session_id: Some(session_id.clone()),
                config: wal_config_snapshot(cfg),
            }) {
                stderr_tracker.writeln_line(&format!("anix: {err}"));
                return Err(err.exit_code());
            }

            let mut max_tid = 0u64;
            let mut entries = Vec::with_capacity(processed.messages.len());
            for msg in processed.messages {
                max_tid = max_tid.max(msg.turn_id());
                let node_id = if replayed.tree_mode {
                    let id = next_node_id;
                    next_node_id = next_node_id.saturating_add(1);
                    Some(id)
                } else {
                    None
                };
                let parent_id = if replayed.tree_mode { head_id } else { None };
                if let Err(err) = wal.append_message(node_id, parent_id, &msg) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
                if replayed.tree_mode {
                    head_id = node_id;
                }
                entries.push(ActiveEntry {
                    node_id,
                    message: msg,
                    entry_usage: TokenUsage::default(),
                });
            }

            return Ok(Self {
                session_id,
                entries,
                assistant_usages: BTreeMap::new(),
                last_checkpoint_turn: 0,
                next_turn_id: max_tid.saturating_add(1).max(1),
                tree_mode: replayed.tree_mode,
                head_id,
                next_node_id,
                next_cycle_id: 1,
                turns_executed: 0,
                run_usage: TokenUsage::default(),
                session_usage: TokenUsage::default(),
                pending_compact: false,
                active_holds: Vec::new(),
                committed_message_ids: HashSet::new(),
            });
        }

        // Resumed / forked / `--leaf` WAL session: preserve frozen prefix verbatim; append
        // trailing `runtime_update` ONLY if contract config drifted along this lineage.
        let mut entries = replayed.entries;
        let mut next_turn_id = replayed.next_turn_id;
        let tree_mode = replayed.tree_mode;
        let mut head_id = replayed.head_id;
        let mut next_node_id = replayed.next_node_id;

        if let Some(prev_cfg) = replayed.latest_config {
            let mut diffs = Vec::new();
            let prev_output = prev_cfg.output.unwrap_or(OutputMode::Explicit);
            if prev_output != cfg.output {
                diffs.push(format!(
                    "output contract switched to {}",
                    match cfg.output {
                        OutputMode::Reply => {
                            "reply (your final text reply is delivered to stdout fd 1; $ANIX_OUT is NOT mounted)"
                        }
                        OutputMode::Explicit => {
                            "explicit (write final deliverables via shell to $ANIX_OUT fd 1; normal text goes to stderr)"
                        }
                        OutputMode::None => {
                            "none (stdout fd 1 is strictly silent; $ANIX_OUT is NOT mounted)"
                        }
                    }
                ));
            }
            let prev_shell_enabled = prev_cfg
                .shell
                .as_deref()
                .is_none_or(|s| !s.trim().eq_ignore_ascii_case("none"));
            if prev_shell_enabled != cfg.shell_enabled() {
                if cfg.shell_enabled() {
                    diffs.push(format!(
                        "shell capability enabled (`{}`; `{}` tool is now available)",
                        cfg.shell, cfg.tool_name
                    ));
                } else {
                    diffs.push("shell capability switched to none (no tools available)".into());
                }
            }
            if prev_cfg.stderr.is_some_and(|s| s != cfg.stderr) {
                diffs.push(format!(
                    "stderr mode switched to {}",
                    match cfg.stderr {
                        StderrMode::Auto => "auto (intermediate/normal text shown on stderr)",
                        StderrMode::Explicit => {
                            "explicit (intermediate/normal text invisible to humans; write human messages to $ANIX_ERR)"
                        }
                    }
                ));
            }

            if !diffs.is_empty() {
                let content = diffs.join("; ");
                let turn_id = next_turn_id;
                next_turn_id = next_turn_id.saturating_add(1);
                let node_id = if tree_mode {
                    let id = next_node_id;
                    next_node_id = next_node_id.saturating_add(1);
                    Some(id)
                } else {
                    None
                };
                let parent_id = if tree_mode { head_id } else { None };
                let ts = posix::timestamp_rfc3339_utc();
                if let Err(err) = wal.append(&WalEvent::RuntimeUpdate {
                    id: node_id,
                    parent_id,
                    turn_id,
                    ts,
                    content: content.clone(),
                    config: wal_config_snapshot(cfg),
                }) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
                if tree_mode {
                    head_id = node_id;
                }
                entries.push(ActiveEntry {
                    node_id,
                    message: IrMessage::system(
                        turn_id,
                        format!("<runtime_update>{content}</runtime_update>"),
                    ),
                    entry_usage: TokenUsage::default(),
                });
            }
        }

        Ok(Self {
            session_id,
            entries,
            assistant_usages: replayed.assistant_usages,
            last_checkpoint_turn: replayed.last_checkpoint_turn,
            next_turn_id,
            tree_mode,
            head_id,
            next_node_id,
            next_cycle_id: 1,
            turns_executed: 0,
            run_usage: TokenUsage::default(),
            session_usage: replayed.session_usage,
            pending_compact: false,
            active_holds: Vec::new(),
            committed_message_ids: replayed.committed_message_ids,
        })
    }

    fn alloc_turn_id(&mut self) -> u64 {
        let tid = self.next_turn_id;
        self.next_turn_id = self.next_turn_id.saturating_add(1);
        tid
    }

    fn alloc_node_id(&mut self) -> Option<u64> {
        if self.tree_mode {
            let id = self.next_node_id;
            self.next_node_id = self.next_node_id.saturating_add(1);
            Some(id)
        } else {
            None
        }
    }

    fn current_parent_id(&self) -> Option<u64> {
        if self.tree_mode { self.head_id } else { None }
    }

    fn reported_leaf_id(&self, cfg: &ResolvedConfig) -> Option<u64> {
        if cfg.context.is_some() && self.tree_mode {
            self.head_id
        } else {
            None
        }
    }

    fn alloc_cycle_id(&mut self) -> u64 {
        let cid = self.next_cycle_id;
        self.next_cycle_id = self.next_cycle_id.saturating_add(1);
        cid
    }

    fn messages_cloned(&self) -> Vec<IrMessage> {
        self.entries.iter().map(|e| e.message.clone()).collect()
    }

    fn push_message(
        &mut self,
        msg: IrMessage,
        usage: Option<TokenUsage>,
        message_ids: Vec<String>,
        wal: &mut WalStore,
        stderr_tracker: &StderrTracker,
    ) -> Result<(), ExitCode> {
        let node_id = self.alloc_node_id();
        let parent_id = self.current_parent_id();
        if let Err(err) =
            wal.append_message_with_meta(node_id, parent_id, &msg, usage, message_ids.clone())
        {
            stderr_tracker.writeln_line(&format!("anix: {err}"));
            return Err(err.exit_code());
        }
        for mid in message_ids {
            if !mid.is_empty() {
                self.committed_message_ids.insert(mid);
            }
        }
        if self.tree_mode {
            self.head_id = node_id;
        }
        self.entries.push(ActiveEntry {
            node_id,
            message: msg,
            entry_usage: usage.unwrap_or_default(),
        });
        Ok(())
    }

    fn push_user_message(
        &mut self,
        text: &str,
        message_ids: Vec<String>,
        wal: &mut WalStore,
        stderr_tracker: &StderrTracker,
    ) -> Result<(), ExitCode> {
        let tid = self.alloc_turn_id();
        let msg = IrMessage::user(tid, text);
        self.push_message(msg, None, message_ids, wal, stderr_tracker)
    }

    fn push_runtime_update(
        &mut self,
        cfg: &ResolvedConfig,
        content: String,
        wal: &mut WalStore,
        stderr_tracker: &StderrTracker,
    ) -> Result<(), ExitCode> {
        let turn_id = self.alloc_turn_id();
        let node_id = self.alloc_node_id();
        let parent_id = self.current_parent_id();
        let ts = posix::timestamp_rfc3339_utc();
        if let Err(err) = wal.append(&WalEvent::RuntimeUpdate {
            id: node_id,
            parent_id,
            turn_id,
            ts,
            content: content.clone(),
            config: wal_config_snapshot(cfg),
        }) {
            stderr_tracker.writeln_line(&format!("anix: {err}"));
            return Err(err.exit_code());
        }
        if self.tree_mode {
            self.head_id = node_id;
        }
        self.entries.push(ActiveEntry {
            node_id,
            message: IrMessage::system(
                turn_id,
                format!("<runtime_update>{content}</runtime_update>"),
            ),
            entry_usage: TokenUsage::default(),
        });
        Ok(())
    }

    fn register_hold(&mut self, id: String, deadline_sec: u64, label: Option<String>) {
        let deadline = Instant::now() + Duration::from_secs(deadline_sec.max(1));
        if let Some(existing) = self.active_holds.iter_mut().find(|h| h.id == id) {
            existing.deadline = deadline;
            existing.label = label;
        } else {
            self.active_holds.push(ActiveHold {
                id,
                deadline,
                label,
            });
        }
    }

    fn release_hold(&mut self, id: &str) -> Option<ActiveHold> {
        let pos = self.active_holds.iter().position(|h| h.id == id)?;
        Some(self.active_holds.remove(pos))
    }

    fn earliest_hold_deadline(&self) -> Option<Instant> {
        self.active_holds.iter().map(|h| h.deadline).min()
    }

    fn drain_expired_holds(&mut self, now: Instant) -> Vec<ActiveHold> {
        let mut expired = Vec::new();
        let mut i = 0;
        while i < self.active_holds.len() {
            if self.active_holds[i].deadline <= now {
                expired.push(self.active_holds.remove(i));
            } else {
                i += 1;
            }
        }
        expired
    }

    fn current_context_tokens(&self) -> u64 {
        let msgs = self.messages_cloned();
        compact::estimate_context_tokens(&msgs, &self.assistant_usages)
    }

    fn build_ir_stream(&self, cfg: &ResolvedConfig) -> IrStream {
        IrStream {
            header: make_ir_header(cfg),
            messages: self.messages_cloned(),
        }
    }
}

/// Bracket a single inference cycle with `cycle_start` and commit-barrier `cycle_end` Trace events.
#[allow(clippy::too_many_arguments)]
fn execute_tracked_cycle(
    source: &str,
    inbox_commit: Option<(&InboxQueue, &[InboxMessage])>,
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
    emit_cycle_usage_stderr: bool,
) -> CycleOutcome {
    let cycle_id = state.alloc_cycle_id();
    let start_turn_id = state.next_turn_id;
    let usage_before = state.run_usage.normalized_5();

    let mut batch_message_ids: Vec<String> = inbox_commit
        .map(|(_, msgs)| msgs.iter().map(|m| m.message_id().to_owned()).collect())
        .unwrap_or_default();

    let mut start_fields = Map::new();
    start_fields.insert("cycle_id".into(), Value::from(cycle_id));
    start_fields.insert("source".into(), Value::String(source.to_owned()));
    start_fields.insert("start_turn_id".into(), Value::from(start_turn_id));
    if !batch_message_ids.is_empty() {
        start_fields.insert(
            "message_ids".into(),
            Value::Array(
                batch_message_ids
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
    }
    tracer.emit_event("cycle_start", start_fields);

    let inbox_queue_ref = inbox_commit.map(|(q, _)| q);
    let mut steered_msgs: Vec<InboxMessage> = Vec::new();
    let mut outcome = run_inference_cycle(
        cfg,
        script,
        state,
        wal,
        sandbox,
        signals,
        provider_pool,
        stderr_tracker,
        tracer,
        inbox_queue_ref,
        &mut steered_msgs,
    );

    for sm in &steered_msgs {
        batch_message_ids.push(sm.message_id().to_owned());
    }

    // Commit barrier (RFC §4.3.3 & §8.4):
    // 1. On success, move all batch + mid-cycle steered `cur/<name>` -> `done/<name>`.
    // 2. Deliver replies to all `Anix-Reply-To` Maildir targets (even on non-zero exit so callers unblock!).
    // 3. Emit `cycle_end`.
    if let Some((queue, msgs)) = inbox_commit {
        let mut all_msgs: Vec<&InboxMessage> = Vec::with_capacity(msgs.len() + steered_msgs.len());
        all_msgs.extend(msgs.iter());
        all_msgs.extend(steered_msgs.iter());

        if outcome.exit_code == ExitCode::Success {
            for msg in &all_msgs {
                if let Err(err) = queue.mark_done(msg) {
                    stderr_tracker.writeln_line(&format!(
                        "anix: failed to move inbox message `{}` to done/: {err}",
                        msg.filename
                    ));
                    outcome.exit_code = ExitCode::RuntimeError;
                    outcome.reason = CycleEndReason::Error;
                    break;
                }
            }
        }
        deliver_batch_replies(
            &all_msgs,
            &cfg.name,
            outcome.exit_code.as_i32(),
            &outcome.delivered_output,
            tracer,
        );
    }

    let run_norm = state.run_usage.normalized_5();
    let cycle_usage = diff_usage(run_norm, usage_before);
    let session_norm = if cfg.context.is_some() {
        state.session_usage.normalized_5()
    } else {
        run_norm
    };
    let ctx_tokens = state.current_context_tokens();
    let context = ContextWatermark {
        tokens: ctx_tokens,
        max_tokens: cfg.max_tokens,
    };

    if emit_cycle_usage_stderr
        && cfg.stderr == StderrMode::Auto
        && (outcome.turns > 0 || !cycle_usage.is_zero())
    {
        emit_usage_line_stderr(
            cfg,
            ctx_tokens,
            cycle_usage,
            session_norm,
            "cycle",
            stderr_tracker,
        );
    }

    let mut end_fields = Map::new();
    end_fields.insert("cycle_id".into(), Value::from(cycle_id));
    end_fields.insert("source".into(), Value::String(source.to_owned()));
    end_fields.insert(
        "reason".into(),
        Value::String(outcome.reason.as_str().into()),
    );
    end_fields.insert("exit_code".into(), Value::from(outcome.exit_code.as_i32()));
    end_fields.insert("turns".into(), Value::from(outcome.turns));
    if !batch_message_ids.is_empty() {
        end_fields.insert(
            "message_ids".into(),
            Value::Array(batch_message_ids.into_iter().map(Value::String).collect()),
        );
    }
    if let Ok(v) = serde_json::to_value(cycle_usage) {
        end_fields.insert("cycle_usage".into(), v);
    }
    if let Ok(v) = serde_json::to_value(run_norm) {
        end_fields.insert("usage".into(), v);
    }
    if let Ok(v) = serde_json::to_value(session_norm) {
        end_fields.insert("session_usage".into(), v);
    }
    if let Ok(v) = serde_json::to_value(context) {
        end_fields.insert("context".into(), v);
    }
    if let Some(lid) = state.reported_leaf_id(cfg) {
        end_fields.insert("leaf_id".into(), Value::from(lid));
    }
    tracer.emit_event("cycle_end", end_fields);

    outcome
}

fn deliver_batch_replies(
    msgs: &[&InboxMessage],
    agent_name: &str,
    exit_code: i32,
    body: &str,
    tracer: &mut Tracer,
) {
    for msg in msgs {
        if let Some(ref reply_dir_str) = msg.envelope.headers.reply_to {
            let reply_dir = PathBuf::from(reply_dir_str);
            if let Err(err) =
                maildir::deliver_reply(&reply_dir, msg.message_id(), agent_name, exit_code, body)
            {
                let mut wf = Map::new();
                wf.insert(
                    "message".into(),
                    Value::String(format!(
                        "failed delivering reply to `{}`: {err}",
                        reply_dir.display()
                    )),
                );
                tracer.emit_event("warning", wf);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_inbox_loop(
    dir: &Path,
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> (ExitCode, bool, String) {
    let mut queue = match InboxQueue::open(dir, Some(signals.wakeup_fd())) {
        Ok(q) => q,
        Err(err) => {
            stderr_tracker.writeln_line(&format!(
                "anix: failed to initialize --inbox `{}`: {err}",
                dir.display()
            ));
            return (ExitCode::RuntimeError, false, String::new());
        }
    };

    let idle_timeout = if cfg.idle_timeout_sec == 0 {
        None
    } else {
        Some(Duration::from_secs(cfg.idle_timeout_sec))
    };
    let debounce = Duration::from_millis(cfg.inbox_debounce_ms);

    let mut printed_any_cycle_usage = false;
    let mut last_output = String::new();

    // 1. Startup `cur/` crash-recovery deduplication (RFC §4.3.3).
    match queue.scan_cur_for_recovery() {
        Ok(cur_msgs) if !cur_msgs.is_empty() => {
            let mut to_process = Vec::new();
            for msg in cur_msgs {
                if state.committed_message_ids.contains(msg.message_id()) {
                    let _ = queue.mark_done(&msg);
                    let mut wf = Map::new();
                    wf.insert(
                        "message".into(),
                        Value::String(format!(
                            "deduplicated already-committed cur/ message `{}` (id={})",
                            msg.filename,
                            msg.message_id()
                        )),
                    );
                    tracer.emit_event("warning", wf);
                } else {
                    to_process.push(msg);
                }
            }
            if !to_process.is_empty() {
                match process_inbox_batch(
                    &queue,
                    &to_process,
                    cfg,
                    script,
                    state,
                    wal,
                    sandbox,
                    signals,
                    provider_pool,
                    stderr_tracker,
                    tracer,
                ) {
                    Ok((code, out, ran_cycle)) => {
                        if ran_cycle {
                            printed_any_cycle_usage = true;
                        }
                        if !out.is_empty() {
                            last_output = out;
                        }
                        if code != ExitCode::Success {
                            return (code, printed_any_cycle_usage, last_output);
                        }
                    }
                    Err(code) => return (code, printed_any_cycle_usage, last_output),
                }
            }
        }
        Ok(_) => {}
        Err(err) => {
            stderr_tracker.writeln_line(&format!("anix: failed scanning --inbox cur/: {err}"));
            return (ExitCode::RuntimeError, false, String::new());
        }
    }

    // 2. Main unified Maildir event loop.
    loop {
        if let Some(code) = signals.termination_exit_code() {
            return (code, printed_any_cycle_usage, last_output);
        }

        let next_hold_deadline = state.earliest_hold_deadline();
        let active_holds_count = state.active_holds.len() as u64;

        let mut dump_snapshot = || {
            stderr_tracker.ensure_newline();
            let ir = state.build_ir_stream(cfg);
            let _ = signal::dump_ir_snapshot(&ir);
        };

        let mut emit_idle = || {
            let mut idle_fields = Map::new();
            idle_fields.insert("idle_timeout_sec".into(), Value::from(cfg.idle_timeout_sec));
            idle_fields.insert("active_holds".into(), Value::from(active_holds_count));
            tracer.emit_event("idle", idle_fields);
        };

        match queue.next_batch(
            idle_timeout,
            next_hold_deadline,
            debounce,
            signals,
            &mut dump_snapshot,
            &mut emit_idle,
        ) {
            Ok(InboxWaitOutcome::Batch(msgs)) => {
                match process_inbox_batch(
                    &queue,
                    &msgs,
                    cfg,
                    script,
                    state,
                    wal,
                    sandbox,
                    signals,
                    provider_pool,
                    stderr_tracker,
                    tracer,
                ) {
                    Ok((code, out, ran_cycle)) => {
                        if ran_cycle {
                            printed_any_cycle_usage = true;
                        }
                        if !out.is_empty() {
                            last_output = out;
                        }
                        if code != ExitCode::Success {
                            return (code, printed_any_cycle_usage, last_output);
                        }
                    }
                    Err(code) => return (code, printed_any_cycle_usage, last_output),
                }
            }
            Ok(InboxWaitOutcome::HoldExpired) => {
                let expired = state.drain_expired_holds(Instant::now());
                if expired.is_empty() {
                    continue;
                }
                for hold in &expired {
                    let mut hf = Map::new();
                    hf.insert("id".into(), Value::String(hold.id.clone()));
                    if let Some(ref lbl) = hold.label {
                        hf.insert("label".into(), Value::String(lbl.clone()));
                    }
                    tracer.emit_event("hold_expired", hf);
                }
                let summary_lines: Vec<String> = expired
                    .iter()
                    .map(|h| match h.label {
                        Some(ref lbl) => format!("Hold expired: id={} ({lbl})", h.id),
                        None => format!("Hold expired: id={}", h.id),
                    })
                    .collect();
                if let Err(code) =
                    state.push_runtime_update(cfg, summary_lines.join("\n"), wal, stderr_tracker)
                {
                    return (code, printed_any_cycle_usage, last_output);
                }
                let outcome = execute_tracked_cycle(
                    "hold_expired",
                    None,
                    cfg,
                    script,
                    state,
                    wal,
                    sandbox,
                    signals,
                    provider_pool,
                    stderr_tracker,
                    tracer,
                    true,
                );
                printed_any_cycle_usage = true;
                if !outcome.delivered_output.is_empty() {
                    last_output = outcome.delivered_output;
                }
                if outcome.exit_code != ExitCode::Success {
                    return (outcome.exit_code, printed_any_cycle_usage, last_output);
                }
            }
            Ok(InboxWaitOutcome::IdleTimeout) => {
                return (ExitCode::Success, printed_any_cycle_usage, last_output);
            }
            Ok(InboxWaitOutcome::Interrupted(code)) => {
                return (code, printed_any_cycle_usage, last_output);
            }
            Err(err) => {
                stderr_tracker.writeln_line(&format!("anix: --inbox error: {err}"));
                return (ExitCode::RuntimeError, printed_any_cycle_usage, last_output);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_inbox_batch(
    queue: &InboxQueue,
    msgs: &[InboxMessage],
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> Result<(ExitCode, String, bool), ExitCode> {
    let mut saw_compact_control = false;
    let mut body_sections: Vec<String> = Vec::new();
    let mut msg_ids: Vec<String> = Vec::with_capacity(msgs.len());

    for msg in msgs {
        msg_ids.push(msg.message_id().to_owned());

        // Release any matching `hold` lease on `Anix-In-Reply-To`.
        if let Some(ref reply_id) = msg.envelope.headers.in_reply_to
            && let Some(released) = state.release_hold(reply_id)
        {
            let mut rf = Map::new();
            rf.insert("id".into(), Value::String(released.id));
            rf.insert("reason".into(), Value::String("in_reply_to".into()));
            if let Some(lbl) = released.label {
                rf.insert("label".into(), Value::String(lbl));
            }
            tracer.emit_event("hold_released", rf);
        }

        if msg
            .envelope
            .headers
            .control
            .as_deref()
            .is_some_and(|op| op.eq_ignore_ascii_case("compact"))
        {
            saw_compact_control = true;
            state.pending_compact = true;
        }

        if !msg.envelope.body.is_empty() {
            body_sections.push(msg.format_for_user_turn());
        }
    }

    // Control-only batch (e.g. `anix send <inbox> --control compact` with no user text):
    if body_sections.is_empty() && saw_compact_control {
        state.pending_compact = false;
        let current_msgs = state.messages_cloned();
        let outcome = compact::run_full_compaction(
            cfg,
            &script.prompt,
            &state.session_id,
            &current_msgs,
            state.next_turn_id,
            state.tree_mode,
            state.head_id,
            state.next_node_id,
            state.session_usage,
            "Anix-Control: compact",
            wal,
            signals,
            provider_pool,
            stderr_tracker,
            tracer,
        );
        match outcome {
            Ok(c_out) => {
                state.entries = c_out.new_entries;
                state.assistant_usages.clear();
                state.last_checkpoint_turn = c_out.checkpoint_turn_id;
                state.next_turn_id = c_out.next_turn_id;
                if state.tree_mode {
                    state.head_id = c_out.head_id;
                    state.next_node_id = c_out.next_node_id;
                }
                if let Some(ref cu) = c_out.compact_usage {
                    state.run_usage.add_assign(cu);
                }
                state.session_usage = c_out.cumulative_usage;
                for m in msgs {
                    let _ = queue.mark_done(m);
                }
                let msg_refs: Vec<&InboxMessage> = msgs.iter().collect();
                deliver_batch_replies(&msg_refs, &cfg.name, 0, "compacted\n", tracer);
                return Ok((ExitCode::Success, String::new(), false));
            }
            Err(err) => {
                stderr_tracker.writeln_line(&format!("anix: {err}"));
                let code = err.exit_code();
                let msg_refs: Vec<&InboxMessage> = msgs.iter().collect();
                deliver_batch_replies(
                    &msg_refs,
                    &cfg.name,
                    code.as_i32(),
                    &err.to_string(),
                    tracer,
                );
                return Ok((code, String::new(), false));
            }
        }
    }

    let combined_user_text = if body_sections.is_empty() {
        msgs.iter()
            .map(InboxMessage::format_for_user_turn)
            .collect::<Vec<_>>()
            .join("\n\n")
    } else {
        body_sections.join("\n\n")
    };

    state.push_user_message(&combined_user_text, msg_ids, wal, stderr_tracker)?;

    let source_label = if msgs.len() == 1 {
        msgs[0].filename.clone()
    } else {
        format!(
            "batch({}:{}..{})",
            msgs.len(),
            msgs[0].filename,
            msgs[msgs.len() - 1].filename
        )
    };

    let outcome = execute_tracked_cycle(
        &source_label,
        Some((queue, msgs)),
        cfg,
        script,
        state,
        wal,
        sandbox,
        signals,
        provider_pool,
        stderr_tracker,
        tracer,
        true,
    );
    Ok((outcome.exit_code, outcome.delivered_output, true))
}

#[allow(clippy::too_many_arguments)]
fn run_inference_cycle(
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
    inbox_queue: Option<&InboxQueue>,
    steered_out: &mut Vec<InboxMessage>,
) -> CycleOutcome {
    let Some(provider_cfg) = cfg.active_provider().cloned() else {
        stderr_tracker.writeln_line(&format!("anix: provider `{}` not found", cfg.provider));
        return CycleOutcome {
            exit_code: ExitCode::UsageError,
            reason: CycleEndReason::Error,
            turns: 0,
            delivered_output: String::new(),
        };
    };

    let shell_bin = if cfg.shell_enabled() {
        cfg.paths.resolve_component_bin(&cfg.shell_program)
    } else {
        PathBuf::new()
    };
    let tools_dir = cfg.paths.effective_tools_dir();

    let mut cycle_turns: u32 = 0;
    let mut captured_explicit_out = Vec::<u8>::new();

    loop {
        if let Some(code) = signals.termination_exit_code() {
            signals.terminate_active_child();
            return CycleOutcome {
                exit_code: code,
                reason: CycleEndReason::Interrupted,
                turns: cycle_turns,
                delivered_output: String::new(),
            };
        }

        if signals.take_snapshot_request() {
            stderr_tracker.ensure_newline();
            let ir = state.build_ir_stream(cfg);
            if let Err(err) = signal::dump_ir_snapshot(&ir) {
                stderr_tracker
                    .writeln_line(&format!("anix: failed to dump SIGUSR2 snapshot: {err}"));
            }
        }

        if cycle_turns >= cfg.max_turns {
            stderr_tracker.writeln_line(&format!(
                "anix: reached max_turns limit ({}) before task completion",
                cfg.max_turns
            ));
            return CycleOutcome {
                exit_code: ExitCode::TimeoutOrBudget,
                reason: CycleEndReason::MaxTurns,
                turns: cycle_turns,
                delivered_output: String::new(),
            };
        }

        // Check `SIGUSR1` / `aclr -s` / `Anix-Control: compact` / auto-compact threshold.
        let sigusr1_compact = signals.take_compact_request();
        let est_tokens = state.current_context_tokens();
        let threshold_hit = cfg
            .effective_compact_threshold()
            .is_some_and(|th| est_tokens >= th);

        if sigusr1_compact || state.pending_compact || threshold_hit {
            let reason = if sigusr1_compact {
                "SIGUSR1"
            } else if state.pending_compact {
                "compact-request"
            } else {
                "auto-threshold"
            };
            state.pending_compact = false;

            let current_msgs = state.messages_cloned();
            match compact::run_full_compaction(
                cfg,
                &script.prompt,
                &state.session_id,
                &current_msgs,
                state.next_turn_id,
                state.tree_mode,
                state.head_id,
                state.next_node_id,
                state.session_usage,
                reason,
                wal,
                signals,
                provider_pool,
                stderr_tracker,
                tracer,
            ) {
                Ok(outcome) => {
                    state.entries = outcome.new_entries;
                    state.assistant_usages.clear();
                    state.last_checkpoint_turn = outcome.checkpoint_turn_id;
                    state.next_turn_id = outcome.next_turn_id;
                    if state.tree_mode {
                        state.head_id = outcome.head_id;
                        state.next_node_id = outcome.next_node_id;
                    }
                    if let Some(ref cu) = outcome.compact_usage {
                        state.run_usage.add_assign(cu);
                    }
                    state.session_usage = outcome.cumulative_usage;
                }
                Err(err) => {
                    if let CompactError::Provider(ref pe) = err
                        && let Some(u) = pe.usage()
                    {
                        state.run_usage.add_assign(&u);
                        state.session_usage.add_assign(&u);
                    }
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    let exit_code = err.exit_code();
                    let end_reason = match err {
                        CompactError::StrictMode(_) => CycleEndReason::GateBlocked,
                        CompactError::Provider(ProviderError::Interrupted(_)) => {
                            CycleEndReason::Interrupted
                        }
                        CompactError::Provider(_) => CycleEndReason::ProviderError,
                        _ => CycleEndReason::Error,
                    };
                    return CycleOutcome {
                        exit_code,
                        reason: end_reason,
                        turns: cycle_turns,
                        delivered_output: String::new(),
                    };
                }
            }
        }

        // Hard `max_tokens` circuit breaker.
        let est_after = state.current_context_tokens();
        if est_after > u64::from(cfg.max_tokens) {
            stderr_tracker.writeln_line(&format!(
                "anix: context token estimate ({est_after}) exceeds max_tokens ({})",
                cfg.max_tokens
            ));
            return CycleOutcome {
                exit_code: ExitCode::TimeoutOrBudget,
                reason: CycleEndReason::MaxTokens,
                turns: cycle_turns,
                delivered_output: String::new(),
            };
        }

        let ir_stream = state.build_ir_stream(cfg);
        if let Err(err) = ir_stream.validate_invariants() {
            stderr_tracker.writeln_line(&format!(
                "anix: internal Canonical IR invariant error: {err}"
            ));
            return CycleOutcome {
                exit_code: ExitCode::RuntimeError,
                reason: CycleEndReason::Error,
                turns: cycle_turns,
                delivered_output: String::new(),
            };
        }

        cycle_turns += 1;
        state.turns_executed += 1;

        let text_stream_mode = match (cfg.output, cfg.shell_enabled()) {
            (OutputMode::Reply, false) => TextStreamMode::StdoutDirect,
            (OutputMode::Reply, true) => TextStreamMode::BufferForReply,
            (OutputMode::Explicit | OutputMode::None, _) => TextStreamMode::StderrIfAuto,
        };

        let response = match provider::invoke_provider(
            &provider_cfg,
            &cfg.model,
            &state.session_id,
            &ir_stream,
            &cfg.paths,
            cfg.stderr,
            true,
            text_stream_mode,
            signals,
            provider_pool,
            stderr_tracker,
            tracer,
        ) {
            Ok(r) => r,
            Err(err) => {
                if let Some(u) = err.usage() {
                    state.run_usage.add_assign(&u);
                    state.session_usage.add_assign(&u);
                }
                if !matches!(err, ProviderError::Interrupted(_)) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                }
                let exit_code = err.exit_code();
                let end_reason = match err {
                    ProviderError::Interrupted(_) => CycleEndReason::Interrupted,
                    ProviderError::StrictMode(_) => CycleEndReason::GateBlocked,
                    _ => CycleEndReason::ProviderError,
                };
                return CycleOutcome {
                    exit_code,
                    reason: end_reason,
                    turns: cycle_turns,
                    delivered_output: String::new(),
                };
            }
        };

        let assistant_turn_id = state.alloc_turn_id();
        if let Some(u) = response.usage {
            state.run_usage.add_assign(&u);
            state.session_usage.add_assign(&u);
            state.assistant_usages.insert(assistant_turn_id, u);
        }

        let assistant_msg = IrMessage::Assistant {
            turn_id: assistant_turn_id,
            text: if response.text.is_empty() && !response.tool_calls.is_empty() {
                None
            } else {
                Some(response.text.clone())
            },
            reasoning: if response.reasoning.is_empty() {
                None
            } else {
                Some(response.reasoning.clone())
            },
            tool_calls: response.tool_calls.clone(),
            provider_opaque: response.provider_opaque.clone(),
            extra: BTreeMap::new(),
        };

        if let Err(code) = state.push_message(
            assistant_msg,
            response.usage,
            Vec::new(),
            wal,
            stderr_tracker,
        ) {
            return CycleOutcome {
                exit_code: code,
                reason: CycleEndReason::Error,
                turns: cycle_turns,
                delivered_output: String::new(),
            };
        }

        // No tool calls => inference task finished!
        if response.tool_calls.is_empty() {
            match text_stream_mode {
                TextStreamMode::BufferForReply => {
                    if cfg.stderr == StderrMode::Auto {
                        stderr_tracker.ensure_newline();
                    }
                    let mut out = io::stdout().lock();
                    let _ = out.write_all(response.text.as_bytes());
                    if posix::isatty(1)
                        && !response.text.is_empty()
                        && !response.text.ends_with('\n')
                    {
                        let _ = out.write_all(b"\n");
                    }
                    let _ = out.flush();
                }
                TextStreamMode::StdoutDirect => {
                    if posix::isatty(1)
                        && !response.text.is_empty()
                        && !response.text.ends_with('\n')
                    {
                        let mut out = io::stdout().lock();
                        let _ = out.write_all(b"\n");
                        let _ = out.flush();
                    }
                }
                TextStreamMode::StderrIfAuto => {}
            }
            let delivered_output = match cfg.output {
                OutputMode::Reply => response.text,
                OutputMode::Explicit => {
                    String::from_utf8_lossy(&captured_explicit_out).into_owned()
                }
                OutputMode::None => String::new(),
            };
            return CycleOutcome {
                exit_code: ExitCode::Success,
                reason: CycleEndReason::Completed,
                turns: cycle_turns,
                delivered_output,
            };
        }

        if text_stream_mode == TextStreamMode::BufferForReply
            && cfg.stderr == StderrMode::Auto
            && !response.text.is_empty()
        {
            stderr_tracker.writeln_dim_if_tty(&response.text);
        }

        let mut folded_this_turn = false;
        for tc in &response.tool_calls {
            if let Some(code) = signals.termination_exit_code() {
                return CycleOutcome {
                    exit_code: code,
                    reason: CycleEndReason::Interrupted,
                    turns: cycle_turns,
                    delivered_output: String::new(),
                };
            }

            if folded_this_turn {
                break;
            }

            if !cfg.shell_enabled() {
                let tid = state.alloc_turn_id();
                let tool_msg = IrMessage::Tool {
                    turn_id: tid,
                    tool_call_id: tc.id.clone(),
                    is_error: true,
                    text: format!(
                        "[t{tid}] exit=2\nError: tool `{}` is unavailable because shell=none",
                        tc.name
                    ),
                    media: Vec::new(),
                    extra: BTreeMap::new(),
                };
                if let Err(code) =
                    state.push_message(tool_msg, None, Vec::new(), wal, stderr_tracker)
                {
                    return CycleOutcome {
                        exit_code: code,
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                        delivered_output: String::new(),
                    };
                }
                continue;
            }

            if tc.name != cfg.tool_name {
                let tid = state.alloc_turn_id();
                let tool_msg = IrMessage::Tool {
                    turn_id: tid,
                    tool_call_id: tc.id.clone(),
                    is_error: true,
                    text: format!(
                        "[t{tid}] exit=2\nError: unknown tool `{}` (only `{}` is available)",
                        tc.name, cfg.tool_name
                    ),
                    media: Vec::new(),
                    extra: BTreeMap::new(),
                };
                if let Err(code) =
                    state.push_message(tool_msg, None, Vec::new(), wal, stderr_tracker)
                {
                    return CycleOutcome {
                        exit_code: code,
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                        delivered_output: String::new(),
                    };
                }
                continue;
            }

            let cmd_str = tc
                .arguments
                .get("cmd")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let exec_timeout_sec = tc.arguments.get("timeout_sec").and_then(Value::as_u64);
            let exec_kill_on_timeout = tc.arguments.get("kill_on_timeout").and_then(Value::as_bool);
            let tool_args_json =
                serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "{}".into());

            if cfg.stderr == StderrMode::Auto {
                stderr_tracker.writeln_dim_if_tty(&format!("[{}] $ {cmd_str}", cfg.tool_name));
            }

            let mut start_fields = Map::new();
            start_fields.insert("tool_call_id".into(), Value::String(tc.id.clone()));
            start_fields.insert("tool_name".into(), Value::String(cfg.tool_name.clone()));
            start_fields.insert("cmd".into(), Value::String(cmd_str.clone()));
            if let Some(t) = exec_timeout_sec {
                start_fields.insert("timeout_sec".into(), Value::from(t));
            }
            if let Some(k) = exec_kill_on_timeout {
                start_fields.insert("kill_on_timeout".into(), Value::Bool(k));
            }
            tracer.emit_event("tool_start", start_fields);

            let mount_anix_out = cfg.output == OutputMode::Explicit;
            let shell_out = match fd::run_shell_command(
                &shell_bin,
                &cfg.shell_prefix_args,
                &tools_dir,
                &cmd_str,
                &tool_args_json,
                &cfg.paths.cwd,
                exec_timeout_sec,
                exec_kill_on_timeout,
                cfg.inbox.as_deref(),
                mount_anix_out,
                sandbox,
                signals,
                stderr_tracker,
            ) {
                Ok(out) => out,
                Err(err) => {
                    stderr_tracker.writeln_line(&format!(
                        "anix: failed to execute shell `{}`: {err}",
                        shell_bin.display()
                    ));
                    return CycleOutcome {
                        exit_code: ExitCode::RuntimeError,
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                        delivered_output: String::new(),
                    };
                }
            };

            if let Some(sig_exit) = shell_out.interrupted_by {
                return CycleOutcome {
                    exit_code: sig_exit,
                    reason: CycleEndReason::Interrupted,
                    turns: cycle_turns,
                    delivered_output: String::new(),
                };
            }

            if !shell_out.anix_out_bytes.is_empty() {
                captured_explicit_out.extend_from_slice(&shell_out.anix_out_bytes);
            }

            for warn in &shell_out.ctl_warnings {
                let mut wf = Map::new();
                wf.insert("message".into(), Value::String(warn.clone()));
                tracer.emit_event("warning", wf);
            }

            if tracer.is_enabled() {
                let mut wf = Map::new();
                wf.insert("tool_call_id".into(), Value::String(tc.id.clone()));
                wf.insert("raw_output".into(), Value::String(shell_out.stdout.clone()));
                tracer.emit_wire("wire_shell_output", wf);
            }

            // RFC §5.4: Exit code 75 (`EX_TEMPFAIL`) from a shell wrapper (e.g. `anix-approve`)
            // signals a Fail-Closed gate abort requesting external/supervisor intervention.
            if shell_out.exit_code == ExitCode::TempFail.as_i32() {
                let detail = shell_out.stdout.trim();
                if !detail.is_empty() {
                    stderr_tracker.writeln_line(&format!("anix: {detail}"));
                } else {
                    stderr_tracker
                        .writeln_line("anix: shell wrapper aborted with exit 75 (EX_TEMPFAIL)");
                }
                return CycleOutcome {
                    exit_code: ExitCode::TempFail,
                    reason: CycleEndReason::GateBlocked,
                    turns: cycle_turns,
                    delivered_output: String::new(),
                };
            }

            let tool_turn_id = state.alloc_turn_id();

            // Process `$ANIX_CTL` frames (`media`, `compact`, `fold`, `hold`, `release`).
            let mut media: Vec<MediaAttachment> = Vec::new();
            let mut extra_warnings: Vec<String> = Vec::new();
            let mut fold_request: Option<(u64, Option<String>)> = None;
            let mut fold_validation_error: Option<String> = None;

            for frame in shell_out.ctl_frames {
                match frame {
                    CtlFrame::Media { mime, path, .. } => {
                        if ALLOWED_MEDIA_MIMES.contains(&mime.as_str()) {
                            media.push(MediaAttachment { mime, path });
                        } else {
                            extra_warnings.push(format!(
                                "ignored media attachment with unsupported MIME `{mime}`"
                            ));
                        }
                    }
                    CtlFrame::Compact => {
                        state.pending_compact = true;
                    }
                    CtlFrame::Fold { from_turn, summary } => {
                        if from_turn <= state.last_checkpoint_turn || from_turn > tool_turn_id {
                            fold_validation_error = Some(format!(
                                "Error: invalid aclr -t turn_id {from_turn} (must satisfy {} < N <= {})",
                                state.last_checkpoint_turn, tool_turn_id
                            ));
                        } else {
                            fold_request = Some((from_turn, summary));
                        }
                    }
                    CtlFrame::Hold {
                        id,
                        deadline_sec,
                        label,
                    } => {
                        let mut hf = Map::new();
                        hf.insert("id".into(), Value::String(id.clone()));
                        hf.insert("deadline_sec".into(), Value::from(deadline_sec));
                        if let Some(ref lbl) = label {
                            hf.insert("label".into(), Value::String(lbl.clone()));
                        }
                        tracer.emit_event("hold_started", hf);
                        state.register_hold(id, deadline_sec, label);
                    }
                    CtlFrame::Release { id } => {
                        if let Some(released) = state.release_hold(&id) {
                            let mut rf = Map::new();
                            rf.insert("id".into(), Value::String(released.id));
                            rf.insert("reason".into(), Value::String("ctl_release".into()));
                            if let Some(lbl) = released.label {
                                rf.insert("label".into(), Value::String(lbl));
                            }
                            tracer.emit_event("hold_released", rf);
                        }
                    }
                }
            }

            for w in &extra_warnings {
                let mut wf = Map::new();
                wf.insert("message".into(), Value::String(w.clone()));
                tracer.emit_event("warning", wf);
            }

            let mut end_fields = Map::new();
            end_fields.insert("turn_id".into(), Value::from(tool_turn_id));
            end_fields.insert("tool_call_id".into(), Value::String(tc.id.clone()));
            end_fields.insert("exit_code".into(), Value::from(shell_out.exit_code));
            end_fields.insert("media_count".into(), Value::from(media.len() as u64));
            tracer.emit_event("tool_end", end_fields);

            let mut output_body = shell_out.stdout;
            if let Some(ref f_err) = fold_validation_error {
                if !output_body.is_empty() && !output_body.ends_with('\n') {
                    output_body.push('\n');
                }
                output_body.push_str(f_err);
            }

            let mut is_error = shell_out.exit_code == ExitCode::TimeoutOrBudget.as_i32()
                || fold_validation_error.is_some();

            // Resolve `aclr -t <N>` (optional auto-summary) BEFORE committing the fold.
            let mut resolved_fold: Option<(u64, String, Option<TokenUsage>)> = None;
            if let Some((req_from, opt_summary)) = fold_request {
                let candidate_tool_msg = IrMessage::Tool {
                    turn_id: tool_turn_id,
                    tool_call_id: tc.id.clone(),
                    is_error,
                    text: format!(
                        "[t{tool_turn_id}] exit={}\n{output_body}",
                        shell_out.exit_code
                    ),
                    media: media.clone(),
                    extra: BTreeMap::new(),
                };
                let mut preview_msgs = state.messages_cloned();
                preview_msgs.push(candidate_tool_msg);
                let aligned_from =
                    wal::align_fold_from_turn(&preview_msgs, req_from).min(assistant_turn_id);

                match opt_summary {
                    Some(explicit_summary) => {
                        resolved_fold = Some((aligned_from, explicit_summary, None));
                    }
                    None => {
                        let tail_slice: Vec<IrMessage> = preview_msgs
                            .into_iter()
                            .filter(|m| m.turn_id() >= aligned_from)
                            .collect();
                        match compact::summarize_tail_fold(
                            cfg,
                            &state.session_id,
                            &tail_slice,
                            signals,
                            provider_pool,
                            stderr_tracker,
                            tracer,
                        ) {
                            Ok((auto_summary, fold_usage)) => {
                                resolved_fold = Some((aligned_from, auto_summary, fold_usage));
                            }
                            Err(CompactError::StrictMode(err)) => {
                                stderr_tracker.writeln_line(&format!("anix: {err}"));
                                return CycleOutcome {
                                    exit_code: ExitCode::TempFail,
                                    reason: CycleEndReason::GateBlocked,
                                    turns: cycle_turns,
                                    delivered_output: String::new(),
                                };
                            }
                            Err(CompactError::Provider(ProviderError::Interrupted(code))) => {
                                return CycleOutcome {
                                    exit_code: code,
                                    reason: CycleEndReason::Interrupted,
                                    turns: cycle_turns,
                                    delivered_output: String::new(),
                                };
                            }
                            Err(err) => {
                                if let CompactError::Provider(ref pe) = err
                                    && let Some(u) = pe.usage()
                                {
                                    state.run_usage.add_assign(&u);
                                    state.session_usage.add_assign(&u);
                                }
                                is_error = true;
                                if !output_body.is_empty() && !output_body.ends_with('\n') {
                                    output_body.push('\n');
                                }
                                output_body.push_str(&format!("Error: auto-fold failed: {err}"));
                            }
                        }
                    }
                }
            }

            let anchored_text = format!(
                "[t{tool_turn_id}] exit={}\n{output_body}",
                shell_out.exit_code
            );

            let tool_msg = IrMessage::Tool {
                turn_id: tool_turn_id,
                tool_call_id: tc.id.clone(),
                is_error,
                text: anchored_text,
                media,
                extra: BTreeMap::new(),
            };

            if let Err(code) = state.push_message(tool_msg, None, Vec::new(), wal, stderr_tracker) {
                return CycleOutcome {
                    exit_code: code,
                    reason: CycleEndReason::Error,
                    turns: cycle_turns,
                    delivered_output: String::new(),
                };
            }

            // Apply validated `aclr -t` tail fold.
            if let Some((aligned_from, summary, fold_usage)) = resolved_fold {
                let to_turn = tool_turn_id;

                let normalized_fold_usage = fold_usage.map(|u| u.normalized_5());
                if let Some(ref fu) = normalized_fold_usage {
                    state.run_usage.add_assign(fu);
                    state.session_usage.add_assign(fu);
                }

                let mut kept = Vec::with_capacity(state.entries.len());
                let mut pruned_total = TokenUsage::default();
                for entry in std::mem::take(&mut state.entries) {
                    if entry.message.turn_id() < aligned_from {
                        kept.push(entry);
                    } else {
                        pruned_total.add_assign(&entry.entry_usage);
                    }
                }
                state.assistant_usages.retain(|&tid, _| tid < aligned_from);

                let pruned_usage = if state.tree_mode && !pruned_total.is_zero() {
                    Some(pruned_total.normalized_5())
                } else {
                    None
                };
                let anchor_parent_id = if state.tree_mode {
                    kept.last().and_then(|e| e.node_id)
                } else {
                    None
                };
                let fold_node_id = state.alloc_node_id();

                let ts = posix::timestamp_rfc3339_utc();
                if let Err(err) = wal.append(&WalEvent::Fold {
                    id: fold_node_id,
                    parent_id: anchor_parent_id,
                    ts,
                    from_turn: aligned_from,
                    to_turn,
                    summary: summary.clone(),
                    fold_usage: normalized_fold_usage,
                    pruned_usage,
                }) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return CycleOutcome {
                        exit_code: err.exit_code(),
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                        delivered_output: String::new(),
                    };
                }

                if state.tree_mode {
                    state.head_id = fold_node_id;
                }
                let mut fold_entry_usage = pruned_total;
                if let Some(ref fu) = normalized_fold_usage {
                    fold_entry_usage.add_assign(fu);
                }
                let folded_msg = IrMessage::user(
                    aligned_from,
                    format!("<folded turns=\"{aligned_from}..{to_turn}\">{summary}</folded>"),
                );
                kept.push(ActiveEntry {
                    node_id: fold_node_id,
                    message: folded_msg,
                    entry_usage: fold_entry_usage,
                });
                state.entries = kept;

                let mut ff = Map::new();
                ff.insert("from_turn".into(), Value::from(aligned_from));
                ff.insert("to_turn".into(), Value::from(to_turn));
                ff.insert("summary".into(), Value::String(summary));
                if cfg.context.is_some()
                    && let Some(nid) = fold_node_id
                {
                    ff.insert("node_id".into(), Value::from(nid));
                }
                if cfg.context.is_some()
                    && let Some(pid) = anchor_parent_id
                {
                    ff.insert("parent_id".into(), Value::from(pid));
                }
                if let Some(fu) = normalized_fold_usage
                    && let Ok(v) = serde_json::to_value(fu)
                {
                    ff.insert("fold_usage".into(), v);
                }
                tracer.emit_event("fold", ff);

                if cfg.stderr == StderrMode::Auto {
                    stderr_tracker
                        .writeln_line(&format!("[anix] folded turns {aligned_from}..{to_turn}"));
                }

                folded_this_turn = true;
            }
        }

        // Mid-cycle Maildir batch steering strictly at the turn boundary:
        // ONLY after ALL paired `tool_calls` of the current assistant turn have finished and
        // committed their `Tool` results to WAL (never between tool calls of the same turn!).
        if let Some(queue) = inbox_queue {
            match queue.try_claim_all_new() {
                Ok(mut steered) if !steered.is_empty() => {
                    let mut steered_ids = Vec::with_capacity(steered.len());
                    let mut body_sections = Vec::new();
                    for msg in &steered {
                        steered_ids.push(msg.message_id().to_owned());
                        if let Some(ref reply_id) = msg.envelope.headers.in_reply_to
                            && let Some(released) = state.release_hold(reply_id)
                        {
                            let mut rf = Map::new();
                            rf.insert("id".into(), Value::String(released.id));
                            rf.insert("reason".into(), Value::String("in_reply_to".into()));
                            if let Some(lbl) = released.label {
                                rf.insert("label".into(), Value::String(lbl));
                            }
                            tracer.emit_event("hold_released", rf);
                        }
                        if msg
                            .envelope
                            .headers
                            .control
                            .as_deref()
                            .is_some_and(|op| op.eq_ignore_ascii_case("compact"))
                        {
                            state.pending_compact = true;
                        }
                        if !msg.envelope.body.is_empty() {
                            body_sections.push(msg.format_for_user_turn());
                        }
                    }
                    if !body_sections.is_empty() {
                        let combined = body_sections.join("\n\n");
                        if let Err(code) = state.push_user_message(
                            &combined,
                            steered_ids.clone(),
                            wal,
                            stderr_tracker,
                        ) {
                            steered_out.append(&mut steered);
                            return CycleOutcome {
                                exit_code: code,
                                reason: CycleEndReason::Error,
                                turns: cycle_turns,
                                delivered_output: String::new(),
                            };
                        }
                    }
                    let mut sf = Map::new();
                    sf.insert("count".into(), Value::from(steered.len() as u64));
                    sf.insert(
                        "message_ids".into(),
                        Value::Array(steered_ids.into_iter().map(Value::String).collect()),
                    );
                    tracer.emit_event("inbox_steered", sf);
                    steered_out.append(&mut steered);
                }
                Ok(_) => {}
                Err(err) => {
                    let mut wf = Map::new();
                    wf.insert(
                        "message".into(),
                        Value::String(format!("mid-cycle inbox drain failed: {err}")),
                    );
                    tracer.emit_event("warning", wf);
                }
            }
        }
    }
}

fn wal_config_snapshot(cfg: &ResolvedConfig) -> WalConfigSnapshot {
    WalConfigSnapshot {
        output: Some(cfg.output),
        shell: Some(cfg.shell.clone()),
        tool_name: Some(cfg.tool_name.clone()),
        stderr: Some(cfg.stderr),
    }
}

fn make_ir_header(cfg: &ResolvedConfig) -> IrHeader {
    IrHeader {
        ir_version: IR_VERSION,
        record_type: HeaderTag::Header,
        tools: if cfg.shell_enabled() {
            vec![IrStream::shell_tool_def(&cfg.tool_name)]
        } else {
            Vec::new()
        },
        max_output_tokens: cfg.max_output_tokens,
        params: cfg.params.clone(),
        meta: IrMeta {
            output_mode: Some(cfg.output),
            stderr_mode: cfg.stderr,
            cwd: cfg.paths.cwd.display().to_string(),
            turn_budget: Some(cfg.max_turns),
            extra: BTreeMap::new(),
        },
        extra: BTreeMap::new(),
    }
}

fn deliver_notify_if_configured(cfg: &ResolvedConfig, exit_code: ExitCode, body: &str) {
    if let Some(ref notify_dir) = cfg.notify {
        let _ = maildir::deliver_notify(
            notify_dir,
            cfg.notify_id.as_deref(),
            &cfg.name,
            exit_code.as_i32(),
            body,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_session(
    cfg: &ResolvedConfig,
    exit_code: ExitCode,
    turns: u32,
    run_usage: TokenUsage,
    session_usage: TokenUsage,
    ctx_tokens: u64,
    leaf_id: Option<u64>,
    final_output: &str,
    emit_usage_stderr: bool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) {
    let run_norm = run_usage.normalized_5();
    let session_norm = if cfg.context.is_some() {
        session_usage.normalized_5()
    } else {
        run_norm
    };
    let context = ContextWatermark {
        tokens: ctx_tokens,
        max_tokens: cfg.max_tokens,
    };

    if emit_usage_stderr && cfg.stderr == StderrMode::Auto && (turns > 0 || !run_usage.is_zero()) {
        emit_usage_line_stderr(
            cfg,
            ctx_tokens,
            run_norm,
            session_norm,
            "run",
            stderr_tracker,
        );
    } else if cfg.stderr == StderrMode::Auto {
        stderr_tracker.ensure_newline();
    }

    deliver_notify_if_configured(cfg, exit_code, final_output);

    let mut fields = Map::new();
    fields.insert("exit_code".into(), Value::from(exit_code.as_i32()));
    fields.insert("turns".into(), Value::from(turns));
    if let Ok(v) = serde_json::to_value(run_norm) {
        fields.insert("usage".into(), v);
    }
    if let Ok(v) = serde_json::to_value(session_norm) {
        fields.insert("session_usage".into(), v);
    }
    if let Ok(v) = serde_json::to_value(context) {
        fields.insert("context".into(), v);
    }
    if let Some(lid) = leaf_id {
        fields.insert("leaf_id".into(), Value::from(lid));
    }
    tracer.emit_event("run_end", fields);
}

fn emit_usage_line_stderr(
    cfg: &ResolvedConfig,
    ctx_tokens: u64,
    primary_usage: TokenUsage,
    session_norm: TokenUsage,
    label: &str,
    stderr_tracker: &StderrTracker,
) {
    let max_tok = u64::from(cfg.max_tokens).max(1);
    let pct = (ctx_tokens.saturating_mul(100) + max_tok / 2) / max_tok;
    let mut line = format!(
        "[usage] ctx {}/{} ({}%) | {} {}",
        ctx_tokens,
        cfg.max_tokens,
        pct,
        label,
        format_usage_summary(primary_usage)
    );
    if cfg.context.is_some() {
        line.push_str(&format!(" | total {}", format_usage_summary(session_norm)));
    }
    stderr_tracker.writeln_dim_if_tty(&line);
}

fn diff_usage(after: TokenUsage, before: TokenUsage) -> TokenUsage {
    TokenUsage {
        input_tokens: after.input_tokens.saturating_sub(before.input_tokens),
        output_tokens: after.output_tokens.saturating_sub(before.output_tokens),
        cached_tokens: Some(after.cached().saturating_sub(before.cached())),
        cache_write_tokens: Some(after.cache_write().saturating_sub(before.cache_write())),
        reasoning_tokens: Some(after.reasoning().saturating_sub(before.reasoning())),
    }
}

fn format_usage_summary(u: TokenUsage) -> String {
    let mut extras = Vec::new();
    if u.cached() > 0 {
        extras.push(format!("cached={}", u.cached()));
    }
    if u.cache_write() > 0 {
        extras.push(format!("write={}", u.cache_write()));
    }
    if u.reasoning() > 0 {
        extras.push(format!("reasoning={}", u.reasoning()));
    }
    if extras.is_empty() {
        format!("in={} out={}", u.input_tokens, u.output_tokens)
    } else {
        format!(
            "in={} out={} ({})",
            u.input_tokens,
            u.output_tokens,
            extras.join(" ")
        )
    }
}
