//! Core inference loop state machine, `msh` tool execution, `$ANIX_CTL` (`fd 6`) frame
//! application, `[vision].bin` / `[approval].bin` integration, `aclr -t <N>` auto-fold,
//! `cycle_start`/`cycle_end`/`idle` Trace observability, and `--inbox` daemon loop (v1.1 §§4–9).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use anix_proto::IR_VERSION;
use anix_proto::ctl::CtlFrame;
use anix_proto::exit_code::ExitCode;
use anix_proto::hooks::{ContextWatermark, OnExitPayload, PostToolPayload};
use anix_proto::ir::{
    HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, MediaAttachment, StderrMode,
};
use anix_proto::provider::TokenUsage;
use anix_proto::wal::{WalConfigSnapshot, WalEvent};
use serde_json::{Map, Value};

use crate::approval::{ApprovalDecision, ApprovalGate};
use crate::compact::{self, CompactError};
use crate::config::ResolvedConfig;
use crate::fd::{self, InboxQueue, StderrTracker};
use crate::frontmatter::AgentScript;
use crate::hooks;
use crate::platform::{self, PreparedSandbox, posix};
use crate::provider::{self, ProviderError, ProviderPool};
use crate::signal::{self, SignalState};
use crate::trace::Tracer;
use crate::vision::{self, VisionBridgeOutcome};
use crate::wal::{self, WalStore};

/// Structured termination reason emitted on `cycle_end` Trace events (v1.1 §9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CycleEndReason {
    Completed,
    MaxTurns,
    MaxTokens,
    ApprovalBlocked,
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
            Self::ApprovalBlocked => "approval_blocked",
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
}

/// Entry point for executing a resolved `*.anix` session (one-shot or `--inbox`).
pub fn execute(cfg: &ResolvedConfig, script: &AgentScript, script_args: &[String]) -> ExitCode {
    // CRITICAL POSIX FD HYGIENE (v1.1 §4.1):
    // 1. NEVER probe `fd 3` implicitly. Only claim an explicitly configured `--trace-fd=<N>` /
    //    `ANIX_TRACE_FD=<N>` FIRST — BEFORE opening any internal descriptors.
    // 2. Drain/close inherited `fd 4..=9` (skipping `cfg.trace_fd` if in that range).
    posix::ignore_sigpipe();
    let trace_file = match cfg.trace_fd {
        Some(fd) => match Tracer::take_configured_fd(fd) {
            Ok(f) => Some(f),
            Err(msg) => {
                eprintln!("anix: {msg}");
                return ExitCode::UsageError;
            }
        },
        None => None,
    };
    let inherited_extra_fds = fd::drain_inherited_extra_fds(cfg.trace_fd);

    let signals = match SignalState::install() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix: failed to install signal handlers: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let stderr_tracker = StderrTracker::new();
    let mut provider_pool = ProviderPool::new();

    // Ensure runtime directory exists with mode 0700 (Section 8.1).
    let _ = posix::ensure_dir_mode(&cfg.paths.runtime_dir, 0o700);

    // Prepare Linux Landlock sandbox policy BEFORE launching any `msh` tool calls (Section 10.2).
    let sandbox: Option<PreparedSandbox> =
        match platform::prepare_sandbox(cfg.sandbox, &cfg.paths.cwd, &cfg.paths.runtime_dir) {
            Ok(sb) => sb,
            Err(msg) => {
                stderr_tracker.writeln_line(&format!("anix: {msg}"));
                return ExitCode::RuntimeError;
            }
        };

    let mut tracer = Tracer::init(&cfg.name, trace_file, &cfg.paths.journal_socket_path());

    // Open WAL context store (`-c <file>`) with non-blocking exclusive `flock` (Section 6.2).
    let mut wal = match cfg.context {
        Some(ref path) => match WalStore::open(path) {
            Ok(store) => store,
            Err(err) => {
                stderr_tracker.writeln_line(&format!("anix: {err}"));
                return err.exit_code();
            }
        },
        None => WalStore::stateless(),
    };

    let replayed = match wal.load_active_window() {
        Ok(r) => r,
        Err(err) => {
            stderr_tracker.writeln_line(&format!("anix: {err}"));
            return err.exit_code();
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
    run_fields.insert("vision".into(), Value::Bool(cfg.vision));
    run_fields.insert("fresh_session".into(), Value::Bool(replayed.is_fresh));
    run_fields.insert("inbox_mode".into(), Value::Bool(cfg.inbox.is_some()));
    tracer.emit_event("run_start", run_fields);

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
                    true,
                    &stderr_tracker,
                    &mut tracer,
                );
                return code;
            }
        };

    let mut approval = ApprovalGate::new(
        cfg.approval,
        cfg.fail_on_deny,
        cfg.safe_commands.clone(),
        cfg.approval_decider.clone(),
        &cfg.name,
        cfg.paths.clone(),
    );

    let mut usage_already_printed_on_cycle = false;

    let exit_code = if let Some(ref inbox_dir) = cfg.inbox {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, true, &signals) {
            Ok(initial) if !initial.is_empty() => {
                if let Err(code) = state.push_user_message(&initial, &mut wal, &stderr_tracker) {
                    provider_pool.shutdown();
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
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
                    &mut approval,
                    sandbox.as_ref(),
                    &signals,
                    &mut provider_pool,
                    &stderr_tracker,
                    &mut tracer,
                    true,
                );
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
                    true,
                    &stderr_tracker,
                    &mut tracer,
                );
                return code;
            }
        }

        let (code, printed_usage) = run_inbox_loop(
            inbox_dir,
            cfg,
            script,
            &mut state,
            &mut wal,
            &mut approval,
            sandbox.as_ref(),
            &signals,
            &mut provider_pool,
            &stderr_tracker,
            &mut tracer,
        );
        usage_already_printed_on_cycle |= printed_usage;
        code
    } else {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, false, &signals) {
            Ok(input) => {
                if !input.is_empty() {
                    if let Err(code) = state.push_user_message(&input, &mut wal, &stderr_tracker) {
                        provider_pool.shutdown();
                        finish_session(
                            cfg,
                            code,
                            state.turns_executed,
                            state.run_usage,
                            state.session_usage,
                            state.current_context_tokens(),
                            true,
                            &stderr_tracker,
                            &mut tracer,
                        );
                        return code;
                    }
                } else if state.messages.len() <= 1
                    && let Err(code) = state.push_user_message("", &mut wal, &stderr_tracker)
                {
                    provider_pool.shutdown();
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
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
                    &mut approval,
                    sandbox.as_ref(),
                    &signals,
                    &mut provider_pool,
                    &stderr_tracker,
                    &mut tracer,
                    false,
                );
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

    // Gracefully close persistent Provider coprocesses (stdin EOF -> exit 0) before final exit.
    provider_pool.shutdown();

    finish_session(
        cfg,
        exit_code,
        state.turns_executed,
        state.run_usage,
        state.session_usage,
        state.current_context_tokens(),
        !usage_already_printed_on_cycle,
        &stderr_tracker,
        &mut tracer,
    );
    exit_code
}

struct SessionState {
    session_id: String,
    messages: Vec<IrMessage>,
    /// Per-turn `usage` for surviving active-window `assistant` messages (never written to IR).
    assistant_usages: BTreeMap<u64, TokenUsage>,
    last_checkpoint_turn: u64,
    next_turn_id: u64,
    next_cycle_id: u64,
    turns_executed: u32,
    /// Cumulative 5-field usage for this process run (including built-in compactor & auto-fold calls).
    run_usage: TokenUsage,
    /// Cross-compaction cumulative 5-field usage across the WAL session.
    session_usage: TokenUsage,
    pending_compact: bool,
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
                &cfg.paths,
                cfg.stderr,
                cfg.vision,
                cfg.vision_bridge.bin.is_some(),
            ) {
                Ok(ir) => ir,
                Err(err) => {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
            };

            let ts = posix::timestamp_rfc3339_utc();
            if let Err(err) = wal.append(&WalEvent::SessionInit {
                ir_version: IR_VERSION,
                ts,
                session_id: Some(session_id.clone()),
                config: WalConfigSnapshot {
                    stderr: Some(cfg.stderr),
                    approval: Some(cfg.approval),
                    vision: Some(cfg.vision),
                },
            }) {
                stderr_tracker.writeln_line(&format!("anix: {err}"));
                return Err(err.exit_code());
            }

            let mut max_tid = 0u64;
            for msg in &processed.messages {
                max_tid = max_tid.max(msg.turn_id());
                if let Err(err) = wal.append_message(msg) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
            }

            return Ok(Self {
                session_id,
                messages: processed.messages,
                assistant_usages: BTreeMap::new(),
                last_checkpoint_turn: 0,
                next_turn_id: max_tid.saturating_add(1).max(1),
                next_cycle_id: 1,
                turns_executed: 0,
                run_usage: TokenUsage::default(),
                session_usage: TokenUsage::default(),
                pending_compact: false,
            });
        }

        // Resumed WAL session: preserve frozen system prefix verbatim and emit trailing
        // `runtime_update` ONLY if contract config (`stderr`, `approval`, or `vision`) drifted.
        let mut messages = replayed.messages;
        let mut next_turn_id = replayed.next_turn_id;

        if let Some(prev_cfg) = replayed.latest_config {
            let mut diffs = Vec::new();
            if prev_cfg.stderr.is_some_and(|s| s != cfg.stderr) {
                diffs.push(format!(
                    "stderr mode switched to {}",
                    match cfg.stderr {
                        StderrMode::Auto => "auto (normal text shown on stderr; deliverables to $ANIX_OUT)",
                        StderrMode::Explicit => "explicit (normal text invisible to humans; write human messages to $ANIX_ERR and deliverables to $ANIX_OUT)",
                    }
                ));
            }
            if prev_cfg.approval.is_some_and(|a| a != cfg.approval) {
                diffs.push(format!("approval policy switched to {:?}", cfg.approval));
            }
            if prev_cfg.vision.is_some_and(|v| v != cfg.vision) {
                diffs.push(format!("vision capability switched to {}", cfg.vision));
            }

            if !diffs.is_empty() {
                let content = diffs.join("; ");
                let turn_id = next_turn_id;
                next_turn_id = next_turn_id.saturating_add(1);
                let ts = posix::timestamp_rfc3339_utc();
                if let Err(err) = wal.append(&WalEvent::RuntimeUpdate {
                    turn_id,
                    ts,
                    content: content.clone(),
                    config: WalConfigSnapshot {
                        stderr: Some(cfg.stderr),
                        approval: Some(cfg.approval),
                        vision: Some(cfg.vision),
                    },
                }) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return Err(err.exit_code());
                }
                messages.push(IrMessage::system(
                    turn_id,
                    format!("<runtime_update>{content}</runtime_update>"),
                ));
            }
        }

        Ok(Self {
            session_id,
            messages,
            assistant_usages: replayed.assistant_usages,
            last_checkpoint_turn: replayed.last_checkpoint_turn,
            next_turn_id,
            next_cycle_id: 1,
            turns_executed: 0,
            run_usage: TokenUsage::default(),
            session_usage: replayed.session_usage,
            pending_compact: false,
        })
    }

    fn alloc_turn_id(&mut self) -> u64 {
        let tid = self.next_turn_id;
        self.next_turn_id = self.next_turn_id.saturating_add(1);
        tid
    }

    fn alloc_cycle_id(&mut self) -> u64 {
        let cid = self.next_cycle_id;
        self.next_cycle_id = self.next_cycle_id.saturating_add(1);
        cid
    }

    fn push_user_message(
        &mut self,
        text: &str,
        wal: &mut WalStore,
        stderr_tracker: &StderrTracker,
    ) -> Result<(), ExitCode> {
        let tid = self.alloc_turn_id();
        let msg = IrMessage::user(tid, text);
        if let Err(err) = wal.append_message(&msg) {
            stderr_tracker.writeln_line(&format!("anix: {err}"));
            return Err(err.exit_code());
        }
        self.messages.push(msg);
        Ok(())
    }

    fn current_context_tokens(&self) -> u64 {
        compact::estimate_context_tokens(&self.messages, &self.assistant_usages)
    }

    fn build_ir_stream(&self, cfg: &ResolvedConfig) -> IrStream {
        IrStream {
            header: make_ir_header(cfg),
            messages: self.messages.clone(),
        }
    }
}

/// Bracket a single inference cycle with `cycle_start` and commit-barrier `cycle_end` Trace events
/// (and optional per-cycle `[usage]` on `fd 2` in `--inbox` mode) (v1.1 §§4.1, 4.2, 9.5).
#[allow(clippy::too_many_arguments)]
fn execute_tracked_cycle(
    source: &str,
    inbox_commit: Option<(&InboxQueue, &fd::InboxMessage)>,
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    approval: &mut ApprovalGate,
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

    let mut start_fields = Map::new();
    start_fields.insert("cycle_id".into(), Value::from(cycle_id));
    start_fields.insert("source".into(), Value::String(source.to_owned()));
    start_fields.insert("start_turn_id".into(), Value::from(start_turn_id));
    tracer.emit_event("cycle_start", start_fields);

    let mut outcome = run_inference_cycle(
        cfg,
        script,
        state,
        wal,
        approval,
        sandbox,
        signals,
        provider_pool,
        stderr_tracker,
        tracer,
    );

    // Commit barrier (v1.1 §4.2 & §9.5): move `cur/<name>` -> `done/<name>` BEFORE emitting `cycle_end`.
    if outcome.exit_code == ExitCode::Success
        && let Some((queue, msg)) = inbox_commit
        && let Err(err) = queue.mark_done(msg)
    {
        stderr_tracker.writeln_line(&format!(
            "anix: failed to move inbox message `{}` to done/: {err}",
            msg.name
        ));
        outcome.exit_code = ExitCode::RuntimeError;
        outcome.reason = CycleEndReason::Error;
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
    tracer.emit_event("cycle_end", end_fields);

    outcome
}

#[allow(clippy::too_many_arguments)]
fn run_inbox_loop(
    dir: &Path,
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    approval: &mut ApprovalGate,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> (ExitCode, bool) {
    let mut queue = match InboxQueue::open(dir, Some(signals.wakeup_fd())) {
        Ok(q) => q,
        Err(err) => {
            stderr_tracker.writeln_line(&format!(
                "anix: failed to initialize --inbox `{}`: {err}",
                dir.display()
            ));
            return (ExitCode::RuntimeError, false);
        }
    };

    let idle_timeout = if cfg.idle_timeout_sec == 0 {
        None
    } else {
        Some(Duration::from_secs(cfg.idle_timeout_sec))
    };

    let mut printed_any_cycle_usage = false;

    loop {
        if let Some(code) = signals.termination_exit_code() {
            return (code, printed_any_cycle_usage);
        }

        let mut dump_snapshot = || {
            stderr_tracker.ensure_newline();
            let ir = state.build_ir_stream(cfg);
            let _ = signal::dump_ir_snapshot(&ir);
        };

        let mut emit_idle = || {
            let mut idle_fields = Map::new();
            idle_fields.insert("idle_timeout_sec".into(), Value::from(cfg.idle_timeout_sec));
            tracer.emit_event("idle", idle_fields);
        };

        match queue.next_message(idle_timeout, signals, &mut dump_snapshot, &mut emit_idle) {
            Ok(Some(msg)) => {
                if let Err(code) =
                    state.push_user_message(&msg.formatted_input, wal, stderr_tracker)
                {
                    return (code, printed_any_cycle_usage);
                }
                let outcome = execute_tracked_cycle(
                    &msg.name,
                    Some((&queue, &msg)),
                    cfg,
                    script,
                    state,
                    wal,
                    approval,
                    sandbox,
                    signals,
                    provider_pool,
                    stderr_tracker,
                    tracer,
                    true,
                );
                printed_any_cycle_usage = true;
                if outcome.exit_code != ExitCode::Success {
                    return (outcome.exit_code, printed_any_cycle_usage);
                }
            }
            Ok(None) => {
                return (
                    signals.termination_exit_code().unwrap_or(ExitCode::Success),
                    printed_any_cycle_usage,
                );
            }
            Err(err) => {
                stderr_tracker.writeln_line(&format!("anix: --inbox error: {err}"));
                return (ExitCode::RuntimeError, printed_any_cycle_usage);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_inference_cycle(
    cfg: &ResolvedConfig,
    script: &AgentScript,
    state: &mut SessionState,
    wal: &mut WalStore,
    approval: &mut ApprovalGate,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    provider_pool: &mut ProviderPool,
    stderr_tracker: &StderrTracker,
    tracer: &mut Tracer,
) -> CycleOutcome {
    let Some(provider_cfg) = cfg.active_provider().cloned() else {
        stderr_tracker.writeln_line(&format!("anix: provider `{}` not found", cfg.provider));
        return CycleOutcome {
            exit_code: ExitCode::UsageError,
            reason: CycleEndReason::Error,
            turns: 0,
        };
    };

    let base_dir = cfg
        .paths
        .project_agent_dir
        .as_deref()
        .and_then(|d| d.parent());
    let shell_bin = cfg.paths.resolve_component_bin(&cfg.shell, base_dir);
    let tools_dir = cfg.paths.effective_tools_dir();

    let mut cycle_turns: u32 = 0;

    loop {
        if let Some(code) = signals.termination_exit_code() {
            signals.terminate_active_child();
            return CycleOutcome {
                exit_code: code,
                reason: CycleEndReason::Interrupted,
                turns: cycle_turns,
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

        // Enforce `max_turns` budget (Section 3.3 / 6.5).
        if cycle_turns >= cfg.max_turns {
            stderr_tracker.writeln_line(&format!(
                "anix: reached max_turns limit ({}) before task completion",
                cfg.max_turns
            ));
            return CycleOutcome {
                exit_code: ExitCode::TimeoutOrBudget,
                reason: CycleEndReason::MaxTurns,
                turns: cycle_turns,
            };
        }

        // Check `SIGUSR1` / `aclr -s` / auto-compact threshold before calling Provider (Section 6.1.2).
        let sigusr1_compact = signals.take_compact_request();
        let est_tokens = state.current_context_tokens();
        let threshold_hit = cfg
            .effective_compact_threshold()
            .is_some_and(|th| est_tokens >= th);

        if sigusr1_compact || state.pending_compact || threshold_hit {
            let reason = if sigusr1_compact {
                "SIGUSR1"
            } else if state.pending_compact {
                "aclr -s"
            } else {
                "auto-threshold"
            };
            state.pending_compact = false;

            match compact::run_full_compaction(
                cfg,
                &script.prompt,
                &state.session_id,
                &state.messages,
                state.next_turn_id,
                state.session_usage,
                reason,
                wal,
                signals,
                provider_pool,
                stderr_tracker,
                tracer,
            ) {
                Ok(outcome) => {
                    state.messages = outcome.new_messages;
                    state.assistant_usages.clear();
                    state.last_checkpoint_turn = outcome.checkpoint_turn_id;
                    state.next_turn_id = outcome.next_turn_id;
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
                        CompactError::Trust(_) => CycleEndReason::ApprovalBlocked,
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
                    };
                }
            }
        }

        // Hard `max_tokens` circuit breaker (Section 6.1.2).
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
            };
        }

        cycle_turns += 1;
        state.turns_executed += 1;

        let response = match provider::invoke_provider(
            &provider_cfg,
            &cfg.model,
            &state.session_id,
            &ir_stream,
            &cfg.paths,
            cfg.stderr,
            true,
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
                    ProviderError::Trust(_) => CycleEndReason::ApprovalBlocked,
                    _ => CycleEndReason::ProviderError,
                };
                return CycleOutcome {
                    exit_code,
                    reason: end_reason,
                    turns: cycle_turns,
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

        if let Err(err) = wal.append_message_with_usage(&assistant_msg, response.usage) {
            stderr_tracker.writeln_line(&format!("anix: {err}"));
            return CycleOutcome {
                exit_code: err.exit_code(),
                reason: CycleEndReason::Error,
                turns: cycle_turns,
            };
        }
        state.messages.push(assistant_msg);

        // No tool calls => inference task finished!
        if response.tool_calls.is_empty() {
            return CycleOutcome {
                exit_code: ExitCode::Success,
                reason: CycleEndReason::Completed,
                turns: cycle_turns,
            };
        }

        // Execute each tool call sequentially; handle `aclr` folds and `vision` policies safely.
        let mut folded_this_turn = false;
        for tc in &response.tool_calls {
            if let Some(code) = signals.termination_exit_code() {
                return CycleOutcome {
                    exit_code: code,
                    reason: CycleEndReason::Interrupted,
                    turns: cycle_turns,
                };
            }

            if folded_this_turn {
                break;
            }

            if tc.name != "msh" {
                let tid = state.alloc_turn_id();
                let tool_msg = IrMessage::Tool {
                    turn_id: tid,
                    tool_call_id: tc.id.clone(),
                    is_error: true,
                    text: format!(
                        "[t{tid}] exit=2\nError: unknown tool `{}` (only `msh` is available)",
                        tc.name
                    ),
                    media: Vec::new(),
                    extra: BTreeMap::new(),
                };
                if let Err(err) = wal.append_message(&tool_msg) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return CycleOutcome {
                        exit_code: err.exit_code(),
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                    };
                }
                state.messages.push(tool_msg);
                continue;
            }

            let cmd_str = tc
                .arguments
                .get("cmd")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();

            let decision = approval.check(&cmd_str, signals);
            let mut app_fields = Map::new();
            app_fields.insert("tool_call_id".into(), Value::String(tc.id.clone()));
            app_fields.insert("cmd".into(), Value::String(cmd_str.clone()));
            match &decision {
                ApprovalDecision::Allow {
                    source,
                    decider,
                    decider_exit_code,
                    ..
                } => {
                    app_fields.insert("decision".into(), Value::String("allow".into()));
                    app_fields.insert("source".into(), Value::String((*source).into()));
                    app_fields.insert("decider".into(), Value::String((*decider).into()));
                    if let Some(ec) = decider_exit_code {
                        app_fields.insert("decider_exit_code".into(), Value::from(*ec));
                    }
                }
                ApprovalDecision::DenyToModel {
                    source,
                    decider,
                    decider_exit_code,
                    ..
                } => {
                    app_fields.insert("decision".into(), Value::String("deny".into()));
                    app_fields.insert("source".into(), Value::String((*source).into()));
                    app_fields.insert("decider".into(), Value::String((*decider).into()));
                    if let Some(ec) = decider_exit_code {
                        app_fields.insert("decider_exit_code".into(), Value::from(*ec));
                    }
                }
                ApprovalDecision::AbortTempFail {
                    decider,
                    decider_exit_code,
                    ..
                } => {
                    app_fields.insert("decision".into(), Value::String("abort_75".into()));
                    app_fields.insert("decider".into(), Value::String((*decider).into()));
                    if let Some(ec) = decider_exit_code {
                        app_fields.insert("decider_exit_code".into(), Value::from(*ec));
                    }
                }
                ApprovalDecision::Interrupted(_) => {
                    app_fields.insert("decision".into(), Value::String("interrupted".into()));
                }
            }
            tracer.emit_event("tool_approval", app_fields);

            let effective_cmd = match decision {
                ApprovalDecision::Allow { cmd, .. } => cmd,
                ApprovalDecision::DenyToModel { message, .. } => {
                    let tid = state.alloc_turn_id();
                    let tool_msg = IrMessage::Tool {
                        turn_id: tid,
                        tool_call_id: tc.id.clone(),
                        is_error: true,
                        text: format!("[t{tid}] exit=75\n{message}"),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    };
                    if let Err(err) = wal.append_message(&tool_msg) {
                        stderr_tracker.writeln_line(&format!("anix: {err}"));
                        return CycleOutcome {
                            exit_code: err.exit_code(),
                            reason: CycleEndReason::Error,
                            turns: cycle_turns,
                        };
                    }
                    state.messages.push(tool_msg);
                    continue;
                }
                ApprovalDecision::AbortTempFail { reason, .. } => {
                    stderr_tracker.writeln_line(&format!("anix: {reason}"));
                    return CycleOutcome {
                        exit_code: ExitCode::TempFail,
                        reason: CycleEndReason::ApprovalBlocked,
                        turns: cycle_turns,
                    };
                }
                ApprovalDecision::Interrupted(code) => {
                    return CycleOutcome {
                        exit_code: code,
                        reason: CycleEndReason::Interrupted,
                        turns: cycle_turns,
                    };
                }
            };

            if cfg.stderr == StderrMode::Auto {
                stderr_tracker.writeln_dim_if_tty(&format!("[msh] $ {effective_cmd}"));
            }

            let mut start_fields = Map::new();
            start_fields.insert("tool_call_id".into(), Value::String(tc.id.clone()));
            start_fields.insert("cmd".into(), Value::String(effective_cmd.clone()));
            tracer.emit_event("tool_start", start_fields);

            let msh_out = match fd::run_msh_command(
                &shell_bin,
                &tools_dir,
                &effective_cmd,
                &cfg.paths.cwd,
                cfg.msh_timeout_sec,
                cfg.approval,
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
                    };
                }
            };

            if let Some(sig_exit) = msh_out.interrupted_by {
                return CycleOutcome {
                    exit_code: sig_exit,
                    reason: CycleEndReason::Interrupted,
                    turns: cycle_turns,
                };
            }

            for warn in &msh_out.ctl_warnings {
                let mut wf = Map::new();
                wf.insert("message".into(), Value::String(warn.clone()));
                tracer.emit_event("warning", wf);
            }

            if tracer.is_enabled() {
                let mut wf = Map::new();
                wf.insert("tool_call_id".into(), Value::String(tc.id.clone()));
                wf.insert("raw_output".into(), Value::String(msh_out.stdout.clone()));
                tracer.emit_wire("wire_msh_output", wf);
            }

            let tool_turn_id = state.alloc_turn_id();

            // Run `post-tool.d/` hooks (Section 9.3).
            let post_payload = PostToolPayload {
                turn_id: tool_turn_id,
                tool_call_id: tc.id.clone(),
                cmd: effective_cmd.clone(),
                exit_code: msh_out.exit_code,
                output: msh_out.stdout,
            };
            let post_payload = match hooks::run_post_tool(post_payload, &cfg.paths) {
                Ok(p) => p,
                Err(err) => {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    let exit_code = err.exit_code();
                    let end_reason = if exit_code == ExitCode::TempFail {
                        CycleEndReason::ApprovalBlocked
                    } else {
                        CycleEndReason::Error
                    };
                    return CycleOutcome {
                        exit_code,
                        reason: end_reason,
                        turns: cycle_turns,
                    };
                }
            };

            // Process `$ANIX_CTL` frames (v1.1 §§5.5, 6.1, 7.2).
            let mut media: Vec<MediaAttachment> = Vec::new();
            let mut extra_text_blocks: Vec<String> = Vec::new();
            let mut vision_error = false;
            let mut fold_request: Option<(u64, Option<String>)> = None;
            let mut fold_validation_error: Option<String> = None;

            for frame in msh_out.ctl_frames {
                match frame {
                    CtlFrame::Media { mime, path, source } => {
                        if cfg.vision {
                            media.push(MediaAttachment { mime, path });
                        } else if cfg.vision_bridge.bin.is_some() {
                            match vision::run_vision_bridge(
                                cfg,
                                &mime,
                                &path,
                                source.as_deref(),
                                signals,
                            ) {
                                VisionBridgeOutcome::Translated(desc) => {
                                    extra_text_blocks.push(desc);
                                }
                                VisionBridgeOutcome::ToolError(err_msg) => {
                                    vision_error = true;
                                    extra_text_blocks.push(err_msg);
                                }
                                VisionBridgeOutcome::TrustViolation(reason) => {
                                    stderr_tracker.writeln_line(&format!("anix: {reason}"));
                                    return CycleOutcome {
                                        exit_code: ExitCode::TempFail,
                                        reason: CycleEndReason::ApprovalBlocked,
                                        turns: cycle_turns,
                                    };
                                }
                                VisionBridgeOutcome::Interrupted(code) => {
                                    return CycleOutcome {
                                        exit_code: code,
                                        reason: CycleEndReason::Interrupted,
                                        turns: cycle_turns,
                                    };
                                }
                            }
                        } else {
                            // v1.1 §5.5: NEVER write `media[]` to message or WAL when `vision=false`.
                            vision_error = true;
                            extra_text_blocks.push(vision::NO_NATIVE_VISION_ERROR.to_owned());
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
                }
            }

            let mut end_fields = Map::new();
            end_fields.insert("turn_id".into(), Value::from(tool_turn_id));
            end_fields.insert("tool_call_id".into(), Value::String(tc.id.clone()));
            end_fields.insert("exit_code".into(), Value::from(post_payload.exit_code));
            end_fields.insert("media_count".into(), Value::from(media.len() as u64));
            tracer.emit_event("tool_end", end_fields);

            let mut output_body = post_payload.output;
            for block in extra_text_blocks {
                if !output_body.is_empty() && !output_body.ends_with('\n') {
                    output_body.push('\n');
                }
                output_body.push_str(&block);
            }
            if let Some(ref f_err) = fold_validation_error {
                if !output_body.is_empty() && !output_body.ends_with('\n') {
                    output_body.push('\n');
                }
                output_body.push_str(f_err);
            }

            let mut is_error = post_payload.exit_code == ExitCode::TimeoutOrBudget.as_i32()
                || fold_validation_error.is_some()
                || vision_error;

            // Resolve `aclr -t <N>` (omitted summary) auto-summary BEFORE committing the fold so
            // any summarization error cleanly aborts the fold and reports `is_error = true` to model.
            let mut resolved_fold: Option<(u64, String, Option<TokenUsage>)> = None;
            if let Some((req_from, opt_summary)) = fold_request {
                let candidate_tool_msg = IrMessage::Tool {
                    turn_id: tool_turn_id,
                    tool_call_id: tc.id.clone(),
                    is_error,
                    text: format!(
                        "[t{tool_turn_id}] exit={}\n{output_body}",
                        post_payload.exit_code
                    ),
                    media: media.clone(),
                    extra: BTreeMap::new(),
                };
                let mut preview_msgs = state.messages.clone();
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
                            Err(CompactError::Trust(err)) => {
                                stderr_tracker.writeln_line(&format!("anix: {err}"));
                                return CycleOutcome {
                                    exit_code: ExitCode::TempFail,
                                    reason: CycleEndReason::ApprovalBlocked,
                                    turns: cycle_turns,
                                };
                            }
                            Err(CompactError::Provider(ProviderError::Interrupted(code))) => {
                                return CycleOutcome {
                                    exit_code: code,
                                    reason: CycleEndReason::Interrupted,
                                    turns: cycle_turns,
                                };
                            }
                            Err(err) => {
                                // v1.1 §6.1: On auto-fold failure, abort fold (no WAL fold event,
                                // no context mutation) and return error as tool result.
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
                post_payload.exit_code
            );

            let tool_msg = IrMessage::Tool {
                turn_id: tool_turn_id,
                tool_call_id: tc.id.clone(),
                is_error,
                text: anchored_text,
                media,
                extra: BTreeMap::new(),
            };

            if let Err(err) = wal.append_message(&tool_msg) {
                stderr_tracker.writeln_line(&format!("anix: {err}"));
                return CycleOutcome {
                    exit_code: err.exit_code(),
                    reason: CycleEndReason::Error,
                    turns: cycle_turns,
                };
            }
            state.messages.push(tool_msg);

            // Apply validated (and optionally auto-summarized) `aclr -t` tail fold (v1.1 §6.1).
            if let Some((aligned_from, summary, fold_usage)) = resolved_fold {
                let to_turn = tool_turn_id;

                // v1.1 §6.1: Count auto-fold usage in `run_usage` / `session_usage`, NEVER in `T_cur`.
                let normalized_fold_usage = fold_usage.map(|u| u.normalized_5());
                if let Some(ref fu) = normalized_fold_usage {
                    state.run_usage.add_assign(fu);
                    state.session_usage.add_assign(fu);
                }

                state.messages.retain(|m| m.turn_id() < aligned_from);
                state.assistant_usages.retain(|&tid, _| tid < aligned_from);
                let folded_msg = IrMessage::user(
                    aligned_from,
                    format!("<folded turns=\"{aligned_from}..{to_turn}\">{summary}</folded>"),
                );
                state.messages.push(folded_msg);

                let ts = posix::timestamp_rfc3339_utc();
                if let Err(err) = wal.append(&WalEvent::Fold {
                    ts,
                    from_turn: aligned_from,
                    to_turn,
                    summary: summary.clone(),
                    fold_usage: normalized_fold_usage,
                }) {
                    stderr_tracker.writeln_line(&format!("anix: {err}"));
                    return CycleOutcome {
                        exit_code: err.exit_code(),
                        reason: CycleEndReason::Error,
                        turns: cycle_turns,
                    };
                }

                let mut ff = Map::new();
                ff.insert("from_turn".into(), Value::from(aligned_from));
                ff.insert("to_turn".into(), Value::from(to_turn));
                ff.insert("summary".into(), Value::String(summary));
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
    }
}

fn make_ir_header(cfg: &ResolvedConfig) -> IrHeader {
    IrHeader {
        ir_version: IR_VERSION,
        record_type: HeaderTag::Header,
        tools: vec![IrStream::msh_tool_def()],
        max_output_tokens: cfg.max_output_tokens,
        params: cfg.params.clone(),
        meta: IrMeta {
            stderr_mode: cfg.stderr,
            cwd: cfg.paths.cwd.display().to_string(),
            turn_budget: Some(cfg.max_turns),
            extra: BTreeMap::new(),
        },
        extra: BTreeMap::new(),
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

    // v1.1 §4.1: Always terminate `fd 2` cleanly on a fresh line before exit; emit static
    // `[usage]` line in `auto` mode (ANSI reset strictly before `\n`). Never emit in `explicit`.
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

    let payload = OnExitPayload {
        exit_code: exit_code.as_i32(),
        turns,
        usage: run_norm,
        session_usage: session_norm,
        context,
        context_file: cfg
            .context
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    };

    hooks::run_on_exit(&payload, &cfg.paths, tracer);

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
