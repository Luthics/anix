//! Core inference loop state machine, `msh` tool execution, `$AGENT_CTL` (`fd 6`) frame
//! application, and Maildir-style `--inbox` / `--idle-timeout` daemon loop (Sections 4, 6, 7, 9).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use os_agent_proto::IR_VERSION;
use os_agent_proto::ctl::CtlFrame;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::hooks::{ContextWatermark, OnExitPayload, PostToolPayload};
use os_agent_proto::ir::{
    HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, MediaAttachment, StderrMode,
};
use os_agent_proto::provider::TokenUsage;
use os_agent_proto::wal::{WalConfigSnapshot, WalEvent};
use serde_json::{Map, Value};

use crate::approval::{ApprovalDecision, ApprovalGate};
use crate::compact;
use crate::config::ResolvedConfig;
use crate::fd::{self, InboxQueue};
use crate::frontmatter::AgentScript;
use crate::hooks;
use crate::platform::{self, PreparedSandbox, posix};
use crate::provider;
use crate::signal::{self, SignalState};
use crate::trace::Tracer;
use crate::wal::{self, WalStore};

/// Entry point for executing a resolved `*.agent` session (one-shot or `--inbox`).
pub fn execute(cfg: &ResolvedConfig, script: &AgentScript, script_args: &[String]) -> ExitCode {
    // CRITICAL POSIX FD HYGIENE:
    // Claim inherited `fd 3` and drain/close inherited `fd 4..=9` FIRST — BEFORE opening any
    // internal descriptors (`signal-hook`, `journal.sock`, WAL `flock`, `inotify`/`kqueue`).
    posix::ignore_sigpipe();
    let inherited_fd3 = Tracer::take_inherited_fd3();
    let inherited_extra_fds = fd::drain_inherited_extra_fds();

    let signals = match SignalState::install() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("agent: failed to install signal handlers: {err}");
            return ExitCode::RuntimeError;
        }
    };

    // Ensure runtime directory exists with mode 0700 (Section 8.1).
    let _ = posix::ensure_dir_mode(&cfg.paths.runtime_dir, 0o700);

    // Prepare Linux Landlock sandbox policy BEFORE launching any `msh` tool calls (Section 10.2).
    let sandbox: Option<PreparedSandbox> =
        match platform::prepare_sandbox(cfg.sandbox, &cfg.paths.cwd, &cfg.paths.runtime_dir) {
            Ok(sb) => sb,
            Err(msg) => {
                eprintln!("agent: {msg}");
                return ExitCode::RuntimeError;
            }
        };

    let mut tracer = Tracer::init(&cfg.name, inherited_fd3, &cfg.paths.journal_socket_path());

    // Open WAL context store (`-c <file>`) with non-blocking exclusive `flock` (Section 6.2).
    let mut wal = match cfg.context {
        Some(ref path) => match WalStore::open(path) {
            Ok(store) => store,
            Err(err) => {
                eprintln!("agent: {err}");
                return err.exit_code();
            }
        },
        None => WalStore::stateless(),
    };

    let replayed = match wal.load_active_window() {
        Ok(r) => r,
        Err(err) => {
            eprintln!("agent: {err}");
            return err.exit_code();
        }
    };

    let mut run_fields = Map::new();
    run_fields.insert("provider".into(), Value::String(cfg.provider.clone()));
    run_fields.insert("model".into(), Value::String(cfg.model.clone()));
    run_fields.insert("fresh_session".into(), Value::Bool(replayed.is_fresh));
    run_fields.insert("inbox_mode".into(), Value::Bool(cfg.inbox.is_some()));
    tracer.emit_event("run_start", run_fields);

    let mut state = match SessionState::init(cfg, script, replayed, &mut wal) {
        Ok(s) => s,
        Err(code) => {
            finish_session(
                cfg,
                code,
                0,
                TokenUsage::default(),
                TokenUsage::default(),
                0,
                &mut tracer,
            );
            return code;
        }
    };

    let mut approval = ApprovalGate::new(cfg.approval, cfg.fail_on_deny, cfg.safe_commands.clone());

    let exit_code = if let Some(ref inbox_dir) = cfg.inbox {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, true, &signals) {
            Ok(initial) if !initial.is_empty() => {
                if let Err(code) = state.push_user_message(&initial, &mut wal) {
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
                        &mut tracer,
                    );
                    return code;
                }
                let code = run_inference_cycle(
                    cfg,
                    script,
                    &mut state,
                    &mut wal,
                    &mut approval,
                    sandbox.as_ref(),
                    &signals,
                    &mut tracer,
                );
                if code != ExitCode::Success {
                    finish_session(
                        cfg,
                        code,
                        state.turns_executed,
                        state.run_usage,
                        state.session_usage,
                        state.current_context_tokens(),
                        &mut tracer,
                    );
                    return code;
                }
            }
            Ok(_) => {}
            Err(err) => {
                let code = if let Some(sig_code) = signals.termination_exit_code() {
                    sig_code
                } else {
                    eprintln!("agent: failed reading initial input FDs: {err}");
                    ExitCode::RuntimeError
                };
                finish_session(
                    cfg,
                    code,
                    state.turns_executed,
                    state.run_usage,
                    state.session_usage,
                    state.current_context_tokens(),
                    &mut tracer,
                );
                return code;
            }
        }

        run_inbox_loop(
            inbox_dir,
            cfg,
            script,
            &mut state,
            &mut wal,
            &mut approval,
            sandbox.as_ref(),
            &signals,
            &mut tracer,
        )
    } else {
        match fd::assemble_initial_inputs(script_args, &inherited_extra_fds, false, &signals) {
            Ok(input) => {
                if !input.is_empty() {
                    if let Err(code) = state.push_user_message(&input, &mut wal) {
                        finish_session(
                            cfg,
                            code,
                            state.turns_executed,
                            state.run_usage,
                            state.session_usage,
                            state.current_context_tokens(),
                            &mut tracer,
                        );
                        return code;
                    }
                } else if state.messages.len() <= 1 {
                    // Fresh session with empty input still needs at least one user message.
                    if let Err(code) = state.push_user_message("", &mut wal) {
                        finish_session(
                            cfg,
                            code,
                            state.turns_executed,
                            state.run_usage,
                            state.session_usage,
                            state.current_context_tokens(),
                            &mut tracer,
                        );
                        return code;
                    }
                }

                run_inference_cycle(
                    cfg,
                    script,
                    &mut state,
                    &mut wal,
                    &mut approval,
                    sandbox.as_ref(),
                    &signals,
                    &mut tracer,
                )
            }
            Err(err) => {
                if let Some(sig_code) = signals.termination_exit_code() {
                    sig_code
                } else {
                    eprintln!("agent: failed reading input FDs: {err}");
                    ExitCode::RuntimeError
                }
            }
        }
    };

    finish_session(
        cfg,
        exit_code,
        state.turns_executed,
        state.run_usage,
        state.session_usage,
        state.current_context_tokens(),
        &mut tracer,
    );
    exit_code
}

struct SessionState {
    messages: Vec<IrMessage>,
    /// Per-turn `usage` for surviving active-window `assistant` messages (never written to IR).
    assistant_usages: BTreeMap<u64, TokenUsage>,
    last_checkpoint_turn: u64,
    next_turn_id: u64,
    turns_executed: u32,
    /// Cumulative 5-field usage for this process run (including built-in compactor calls).
    run_usage: TokenUsage,
    /// Cross-compaction cumulative 5-field usage across the WAL session.
    session_usage: TokenUsage,
    pending_compact: bool,
}

impl SessionState {
    fn init(
        cfg: &ResolvedConfig,
        script: &AgentScript,
        replayed: wal::ReplayedSession,
        wal: &mut WalStore,
    ) -> Result<Self, ExitCode> {
        if replayed.is_fresh {
            // Fresh session: run `pre-context.d/` (including built-ins 00-posix-runtime, 20-skills,
            // 30-tips) and freeze the resulting `turn_id: 0` `role: "system"` prefix (Section 6.2.2).
            let seed = IrStream {
                header: make_ir_header(cfg),
                messages: vec![IrMessage::system(0, &script.prompt)],
            };
            let processed = match hooks::run_pre_context(seed, &cfg.paths, cfg.stderr) {
                Ok(ir) => ir,
                Err(err) => {
                    eprintln!("agent: {err}");
                    return Err(err.exit_code());
                }
            };

            let ts = posix::timestamp_rfc3339_utc();
            if let Err(err) = wal.append(&WalEvent::SessionInit {
                ir_version: IR_VERSION,
                ts,
                config: WalConfigSnapshot {
                    stderr: Some(cfg.stderr),
                    approval: Some(cfg.approval),
                },
            }) {
                eprintln!("agent: {err}");
                return Err(err.exit_code());
            }

            let mut max_tid = 0u64;
            for msg in &processed.messages {
                max_tid = max_tid.max(msg.turn_id());
                if let Err(err) = wal.append_message(msg) {
                    eprintln!("agent: {err}");
                    return Err(err.exit_code());
                }
            }

            return Ok(Self {
                messages: processed.messages,
                assistant_usages: BTreeMap::new(),
                last_checkpoint_turn: 0,
                next_turn_id: max_tid.saturating_add(1).max(1),
                turns_executed: 0,
                run_usage: TokenUsage::default(),
                session_usage: TokenUsage::default(),
                pending_compact: false,
            });
        }

        // Resumed WAL session: preserve frozen system prefix verbatim and emit trailing
        // `runtime_update` ONLY if contract config (`stderr` or `approval`) drifted (Section 6.2.2).
        let mut messages = replayed.messages;
        let mut next_turn_id = replayed.next_turn_id;

        if let Some(prev_cfg) = replayed.latest_config {
            let mut diffs = Vec::new();
            if prev_cfg.stderr.is_some_and(|s| s != cfg.stderr) {
                diffs.push(format!(
                    "stderr mode switched to {}",
                    match cfg.stderr {
                        StderrMode::Auto => "auto (normal text shown on stderr; deliverables to $AGENT_OUT)",
                        StderrMode::Explicit => "explicit (normal text invisible to humans; write human messages to $AGENT_ERR and deliverables to $AGENT_OUT)",
                    }
                ));
            }
            if prev_cfg.approval.is_some_and(|a| a != cfg.approval) {
                diffs.push(format!("approval policy switched to {:?}", cfg.approval));
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
                    },
                }) {
                    eprintln!("agent: {err}");
                    return Err(err.exit_code());
                }
                messages.push(IrMessage::system(
                    turn_id,
                    format!("<runtime_update>{content}</runtime_update>"),
                ));
            }
        }

        Ok(Self {
            messages,
            assistant_usages: replayed.assistant_usages,
            last_checkpoint_turn: replayed.last_checkpoint_turn,
            next_turn_id,
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

    fn push_user_message(&mut self, text: &str, wal: &mut WalStore) -> Result<(), ExitCode> {
        let tid = self.alloc_turn_id();
        let msg = IrMessage::user(tid, text);
        if let Err(err) = wal.append_message(&msg) {
            eprintln!("agent: {err}");
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
    tracer: &mut Tracer,
) -> ExitCode {
    let mut queue = match InboxQueue::open(dir) {
        Ok(q) => q,
        Err(err) => {
            eprintln!(
                "agent: failed to initialize --inbox `{}`: {err}",
                dir.display()
            );
            return ExitCode::RuntimeError;
        }
    };

    let idle_timeout = if cfg.idle_timeout_sec == 0 {
        None
    } else {
        Some(Duration::from_secs(cfg.idle_timeout_sec))
    };

    loop {
        if let Some(code) = signals.termination_exit_code() {
            return code;
        }

        let mut dump_snapshot = || {
            let ir = state.build_ir_stream(cfg);
            let _ = signal::dump_ir_snapshot(&ir);
        };

        match queue.next_message(idle_timeout, signals, &mut dump_snapshot) {
            Ok(Some(msg)) => {
                if let Err(code) = state.push_user_message(&msg.formatted_input, wal) {
                    return code;
                }
                let code = run_inference_cycle(
                    cfg, script, state, wal, approval, sandbox, signals, tracer,
                );
                if code != ExitCode::Success {
                    return code;
                }
                if let Err(err) = queue.mark_done(&msg) {
                    eprintln!(
                        "agent: failed to move inbox message `{}` to done/: {err}",
                        msg.name
                    );
                    return ExitCode::RuntimeError;
                }
            }
            Ok(None) => {
                return signals.termination_exit_code().unwrap_or(ExitCode::Success);
            }
            Err(err) => {
                eprintln!("agent: --inbox error: {err}");
                return ExitCode::RuntimeError;
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
    tracer: &mut Tracer,
) -> ExitCode {
    let Some(provider_cfg) = cfg.active_provider().cloned() else {
        eprintln!("agent: provider `{}` not found", cfg.provider);
        return ExitCode::UsageError;
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
            return code;
        }

        if signals.take_snapshot_request() {
            let ir = state.build_ir_stream(cfg);
            if let Err(err) = signal::dump_ir_snapshot(&ir) {
                eprintln!("agent: failed to dump SIGUSR2 snapshot: {err}");
            }
        }

        // Enforce `max_turns` budget (Section 3.3 / 6.4).
        if cycle_turns >= cfg.max_turns {
            eprintln!(
                "agent: reached max_turns limit ({}) before task completion",
                cfg.max_turns
            );
            return ExitCode::TimeoutOrBudget;
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
                &state.messages,
                state.next_turn_id,
                state.session_usage,
                reason,
                wal,
                signals,
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
                    if let compact::CompactError::Provider(ref pe) = err
                        && let Some(u) = pe.usage()
                    {
                        state.run_usage.add_assign(&u);
                        state.session_usage.add_assign(&u);
                    }
                    eprintln!("agent: {err}");
                    return err.exit_code();
                }
            }
        }

        // Hard `max_tokens` circuit breaker (Section 6.1.2).
        let est_after = state.current_context_tokens();
        if est_after > u64::from(cfg.max_tokens) {
            eprintln!(
                "agent: context token estimate ({est_after}) exceeds max_tokens ({})",
                cfg.max_tokens
            );
            return ExitCode::TimeoutOrBudget;
        }

        let ir_stream = state.build_ir_stream(cfg);
        if let Err(err) = ir_stream.validate_invariants() {
            eprintln!("agent: internal Canonical IR invariant error: {err}");
            return ExitCode::RuntimeError;
        }

        cycle_turns += 1;
        state.turns_executed += 1;

        let response = match provider::invoke_provider(
            &provider_cfg,
            &cfg.model,
            &ir_stream,
            &cfg.paths,
            cfg.stderr,
            true,
            signals,
            tracer,
        ) {
            Ok(r) => r,
            Err(err) => {
                if let Some(u) = err.usage() {
                    state.run_usage.add_assign(&u);
                    state.session_usage.add_assign(&u);
                }
                if !matches!(err, provider::ProviderError::Interrupted(_)) {
                    eprintln!("agent: {err}");
                }
                return err.exit_code();
            }
        };

        // Persist assistant turn cleanly (Section 6.2 & 9.2): `usage` is stored in WAL &
        // `state.assistant_usages` for `T_cur`, and strictly stripped from Canonical IR.
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
            eprintln!("agent: {err}");
            return err.exit_code();
        }
        state.messages.push(assistant_msg);

        // No tool calls => inference task finished!
        if response.tool_calls.is_empty() {
            return ExitCode::Success;
        }

        // Execute each tool call sequentially; handle `aclr` folds safely.
        let mut folded_this_turn = false;
        for tc in &response.tool_calls {
            if let Some(code) = signals.termination_exit_code() {
                return code;
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
                    eprintln!("agent: {err}");
                    return err.exit_code();
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
                ApprovalDecision::Allow { source, .. } => {
                    app_fields.insert("decision".into(), Value::String("allow".into()));
                    app_fields.insert("source".into(), Value::String((*source).into()));
                }
                ApprovalDecision::DenyToModel { source, .. } => {
                    app_fields.insert("decision".into(), Value::String("deny".into()));
                    app_fields.insert("source".into(), Value::String((*source).into()));
                }
                ApprovalDecision::AbortExit42 { .. } => {
                    app_fields.insert("decision".into(), Value::String("abort_42".into()));
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
                        text: format!("[t{tid}] exit=42\n{message}"),
                        media: Vec::new(),
                        extra: BTreeMap::new(),
                    };
                    if let Err(err) = wal.append_message(&tool_msg) {
                        eprintln!("agent: {err}");
                        return err.exit_code();
                    }
                    state.messages.push(tool_msg);
                    continue;
                }
                ApprovalDecision::AbortExit42 { reason } => {
                    eprintln!("agent: {reason}");
                    return ExitCode::PermissionDenied;
                }
                ApprovalDecision::Interrupted(code) => {
                    return code;
                }
            };

            if cfg.stderr == StderrMode::Auto {
                if posix::isatty(2) {
                    eprintln!("\x1b[2m[msh] $ {effective_cmd}\x1b[0m");
                } else {
                    eprintln!("[msh] $ {effective_cmd}");
                }
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
            ) {
                Ok(out) => out,
                Err(err) => {
                    eprintln!(
                        "agent: failed to execute shell `{}`: {err}",
                        shell_bin.display()
                    );
                    return ExitCode::RuntimeError;
                }
            };

            if let Some(sig_exit) = msh_out.interrupted_by {
                // Discard uncommitted half-turn on SIGINT/SIGTERM (Section 6.3).
                return sig_exit;
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
                    eprintln!("agent: {err}");
                    return err.exit_code();
                }
            };

            // Separate `$AGENT_CTL` media attachments vs compaction/fold control frames (Section 7.2).
            let mut media: Vec<MediaAttachment> = Vec::new();
            let mut fold_request: Option<(u64, String)> = None;
            let mut fold_validation_error: Option<String> = None;

            for frame in msh_out.ctl_frames {
                match frame {
                    CtlFrame::Media { mime, path, .. } => {
                        media.push(MediaAttachment { mime, path });
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
            if let Some(ref f_err) = fold_validation_error {
                if !output_body.is_empty() && !output_body.ends_with('\n') {
                    output_body.push('\n');
                }
                output_body.push_str(f_err);
            }

            // Section 6.1.1 & 7.1.5: Anchor `[t<turn_id>] exit=<code>\n` prepended by `agent`;
            // single `msh` timeout (`exit 124`) sets `is_error = true` without killing `agent`.
            let is_error = post_payload.exit_code == ExitCode::TimeoutOrBudget.as_i32()
                || fold_validation_error.is_some();
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
                eprintln!("agent: {err}");
                return err.exit_code();
            }
            state.messages.push(tool_msg);

            // Apply `aclr -t <N> "<summary>"` tail fold with Tool-Pair Closure (Section 6.1.3).
            if let Some((req_from, summary)) = fold_request {
                let aligned_from =
                    wal::align_fold_from_turn(&state.messages, req_from).min(assistant_turn_id);
                let to_turn = tool_turn_id;

                // Pop all turns >= aligned_from (including the assistant(tool_call) that invoked aclr).
                // Dropping folded turn IDs from `assistant_usages` automatically rolls `T_cur` back
                // to the last surviving assistant before the fold boundary (Section 6.1.2).
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
                }) {
                    eprintln!("agent: {err}");
                    return err.exit_code();
                }

                let mut ff = Map::new();
                ff.insert("from_turn".into(), Value::from(aligned_from));
                ff.insert("to_turn".into(), Value::from(to_turn));
                ff.insert("summary".into(), Value::String(summary));
                tracer.emit_event("fold", ff);

                if cfg.stderr == StderrMode::Auto {
                    eprintln!("[agent] folded turns {aligned_from}..{to_turn}");
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

fn finish_session(
    cfg: &ResolvedConfig,
    exit_code: ExitCode,
    turns: u32,
    run_usage: TokenUsage,
    session_usage: TokenUsage,
    ctx_tokens: u64,
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

    // Section 4.1: In `auto` mode, emit ONE static ANSI Dim `[usage]` summary line on `fd 2`
    // before exit (no cursor movement / dynamic refresh). In `explicit` mode (`-q`), NEVER emit.
    if cfg.stderr == StderrMode::Auto && (turns > 0 || !run_usage.is_zero()) {
        let max_tok = u64::from(cfg.max_tokens).max(1);
        let pct = (ctx_tokens.saturating_mul(100) + max_tok / 2) / max_tok;
        let mut line = format!(
            "[usage] ctx {}/{} ({}%) | run {}",
            ctx_tokens,
            cfg.max_tokens,
            pct,
            format_usage_summary(run_norm)
        );
        if cfg.context.is_some() {
            line.push_str(&format!(" | total {}", format_usage_summary(session_norm)));
        }
        if posix::isatty(2) {
            eprintln!("\x1b[2m{line}\x1b[0m");
        } else {
            eprintln!("{line}");
        }
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
