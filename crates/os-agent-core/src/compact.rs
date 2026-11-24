//! Token budget tracking, auto-compaction trigger, and `[compact]` strategy resolution
//! (external `[compact].bin` vs built-in Provider summarizer) (Section 6.1).

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::thread;

use os_agent_proto::IR_VERSION;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, StderrMode};
use os_agent_proto::wal::{WalConfigSnapshot, WalEvent};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::config::ResolvedConfig;
use crate::hooks::{self, HookError};
use crate::platform::posix;
use crate::provider::{self, ProviderError};
use crate::signal::SignalState;
use crate::trace::Tracer;
use crate::trust::{self, TrustError};
use crate::wal::{WalError, WalStore};

const DEFAULT_COMPACT_PROMPT: &str = "\
Summarize the conversation so far into a concise, factual checkpoint. Preserve all user goals, key decisions, verified facts, file paths, command outcomes, constraints, and remaining open tasks. Output plain text only.";

#[derive(Debug, Error)]
pub enum CompactError {
    #[error(transparent)]
    Trust(#[from] TrustError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Hook(#[from] HookError),
    #[error(transparent)]
    Wal(#[from] WalError),
    #[error("compaction failed: {0}")]
    Failed(String),
}

impl CompactError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Trust(_) => ExitCode::PermissionDenied,
            Self::Provider(e) => e.exit_code(),
            Self::Hook(e) => e.exit_code(),
            Self::Wal(e) => e.exit_code(),
            Self::Failed(_) => ExitCode::RuntimeError,
        }
    }
}

/// Conservative token estimator per Section 6.1.2:
/// `last_usage_total + ceil(bytes_added_since_last_usage / 3)`.
#[derive(Debug, Default, Clone, Copy)]
pub struct TokenBudgetTracker {
    last_usage_total: Option<u64>,
    bytes_since_usage: usize,
}

impl TokenBudgetTracker {
    pub fn record_usage(&mut self, input_tokens: u64, output_tokens: u64) {
        self.last_usage_total = Some(input_tokens.saturating_add(output_tokens));
        self.bytes_since_usage = 0;
    }

    pub fn add_message_bytes(&mut self, msg: &IrMessage) {
        if let Ok(s) = serde_json::to_string(msg) {
            self.bytes_since_usage = self.bytes_since_usage.saturating_add(s.len());
        }
    }

    pub fn reset(&mut self) {
        self.last_usage_total = None;
        self.bytes_since_usage = 0;
    }

    /// Estimate `T_cur` for the current active message window.
    pub fn estimate_current_tokens(&self, messages: &[IrMessage]) -> u64 {
        match self.last_usage_total {
            Some(base) => base.saturating_add((self.bytes_since_usage as u64).div_ceil(3)),
            None => {
                let total_bytes: u64 = messages
                    .iter()
                    .filter_map(|m| serde_json::to_string(m).ok())
                    .map(|s| s.len() as u64)
                    .sum();
                total_bytes.div_ceil(3)
            }
        }
    }
}

/// Result of a completed full compaction checkpoint.
#[derive(Debug)]
pub struct CompactionOutcome {
    pub new_messages: Vec<IrMessage>,
    pub checkpoint_turn_id: u64,
    pub next_turn_id: u64,
    pub summary_len: usize,
}

/// Execute full compaction (auto-compact, `aclr -s`, or `SIGUSR1`), refresh frozen
/// `role: "system"` via `pre-context.d/`, and append a WAL `checkpoint` section (Section 6.1 & 6.2).
#[allow(clippy::too_many_arguments)]
pub fn run_full_compaction(
    cfg: &ResolvedConfig,
    script_prompt: &str,
    current_messages: &[IrMessage],
    next_turn_id: u64,
    reason: &str,
    wal: &mut WalStore,
    signals: &SignalState,
    tracer: &mut Tracer,
) -> Result<CompactionOutcome, CompactError> {
    if cfg.stderr == StderrMode::Auto {
        eprintln!("[agent] compacting context ({reason})...");
    }

    // Build tool-less Canonical IR copy for compaction (Section 6.1.4: no `msh` tool).
    let compact_ir = IrStream {
        header: IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools: Vec::new(),
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                stderr_mode: cfg.stderr,
                cwd: cfg.paths.cwd.display().to_string(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        },
        messages: current_messages.to_vec(),
    };

    let summary = if let Some(ref bin_spec) = cfg.compact.bin {
        run_external_compactor(cfg, bin_spec, &compact_ir)?
    } else {
        run_builtin_compactor(cfg, compact_ir, signals, tracer)?
    };

    let summary = summary.trim().to_owned();
    if summary.is_empty() {
        return Err(CompactError::Failed(
            "compactor returned an empty summary".into(),
        ));
    }

    // Refresh frozen `role: "system"` prefix by re-running `pre-context.d/` (Section 6.2.2).
    let checkpoint_turn = next_turn_id;
    let seed_ir = IrStream {
        header: IrHeader {
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
        },
        messages: vec![IrMessage::system(checkpoint_turn, script_prompt)],
    };
    let refreshed_ir = hooks::run_pre_context(seed_ir, &cfg.paths, cfg.stderr)?;
    let fresh_system_text = refreshed_ir
        .messages
        .iter()
        .find_map(|m| match m {
            IrMessage::System { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| script_prompt.to_owned());

    let system_msg = IrMessage::system(checkpoint_turn, fresh_system_text);
    let summary_turn = checkpoint_turn.saturating_add(1);
    let summary_msg = IrMessage::user(
        summary_turn,
        format!("<checkpoint>\n{summary}\n</checkpoint>"),
    );

    let ts = posix::timestamp_rfc3339_utc();
    wal.append(&WalEvent::Checkpoint {
        ir_version: IR_VERSION,
        turn_id: checkpoint_turn,
        ts,
        config: WalConfigSnapshot {
            stderr: Some(cfg.stderr),
            approval: Some(cfg.approval),
        },
    })?;
    wal.append_message(&system_msg)?;
    wal.append_message(&summary_msg)?;

    let mut fields = Map::new();
    fields.insert("reason".into(), Value::String(reason.to_owned()));
    fields.insert("checkpoint_turn".into(), Value::from(checkpoint_turn));
    fields.insert("summary_bytes".into(), Value::from(summary.len() as u64));
    tracer.emit_event("compact", fields);

    Ok(CompactionOutcome {
        new_messages: vec![system_msg, summary_msg],
        checkpoint_turn_id: checkpoint_turn,
        next_turn_id: summary_turn.saturating_add(1),
        summary_len: summary.len(),
    })
}

fn run_external_compactor(
    cfg: &ResolvedConfig,
    bin_spec: &str,
    compact_ir: &IrStream,
) -> Result<String, CompactError> {
    let base_dir = cfg
        .paths
        .project_agent_dir
        .as_deref()
        .and_then(|d| d.parent());
    let bin_path = cfg.paths.resolve_component_bin(bin_spec, base_dir);

    if cfg.compact.bin_from_project {
        trust::verify_trusted(&bin_path, &cfg.paths)?;
    }

    let ir_jsonl = compact_ir
        .to_jsonl()
        .map_err(|e| CompactError::Failed(format!("failed to serialize IR for compactor: {e}")))?;

    let mut child = Command::new(&bin_path)
        .current_dir(&cfg.paths.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            CompactError::Failed(format!(
                "failed to execute [compact].bin `{}`: {e}",
                bin_path.display()
            ))
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        let bytes = ir_jsonl.into_bytes();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }

    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(ref mut p) = out_pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    });
    let err_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(ref mut p) = err_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let status = child
        .wait()
        .map_err(|e| CompactError::Failed(format!("failed waiting on [compact].bin: {e}")))?;
    let stdout_bytes = out_handle.join().unwrap_or_default();
    let stderr_str = err_handle.join().unwrap_or_default();

    if !status.success() {
        return Err(CompactError::Failed(format!(
            "[compact].bin exited with {}: {}",
            status.code().unwrap_or(1),
            stderr_str.trim()
        )));
    }

    let stdout = String::from_utf8(stdout_bytes)
        .map_err(|_| CompactError::Failed("[compact].bin produced non-UTF-8 stdout".into()))?;
    if stdout.trim().is_empty() {
        return Err(CompactError::Failed(
            "[compact].bin produced empty stdout".into(),
        ));
    }

    Ok(stdout)
}

fn run_builtin_compactor(
    cfg: &ResolvedConfig,
    mut compact_ir: IrStream,
    signals: &SignalState,
    tracer: &mut Tracer,
) -> Result<String, CompactError> {
    let provider_name = cfg.compact.provider.as_deref().unwrap_or(&cfg.provider);
    let provider_cfg = cfg.providers.get(provider_name).ok_or_else(|| {
        CompactError::Failed(format!(
            "[compact].provider `{provider_name}` is not defined in [providers.{provider_name}]"
        ))
    })?;
    let model = cfg.compact.model.as_deref().unwrap_or(&cfg.model);

    // Clear provider_opaque if compacting through a different provider (Section 9.1 / 9.2).
    if provider_name != cfg.provider {
        for msg in &mut compact_ir.messages {
            if let IrMessage::Assistant {
                provider_opaque, ..
            } = msg
            {
                *provider_opaque = None;
            }
        }
    }

    let prompt = if let Some(ref inline) = cfg.compact.prompt {
        inline.clone()
    } else if let Some(ref file) = cfg.compact.prompt_file {
        fs::read_to_string(file).map_err(|e| {
            CompactError::Failed(format!(
                "failed to read [compact].prompt_file `{}`: {e}",
                file.display()
            ))
        })?
    } else {
        DEFAULT_COMPACT_PROMPT.to_owned()
    };

    let next_tid = compact_ir
        .messages
        .last()
        .map(|m| m.turn_id().saturating_add(1))
        .unwrap_or(1);
    compact_ir.messages.push(IrMessage::user(next_tid, prompt));

    let res = provider::invoke_provider(
        provider_cfg,
        model,
        &compact_ir,
        &cfg.paths,
        cfg.stderr,
        false,
        signals,
        tracer,
    )?;

    Ok(res.text)
}
