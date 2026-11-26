//! Built-in `00-posix-runtime` contract hook and `pre-context.d/` external hook runner
//! protected by POSIX `StrictModes` (RFC §§4.4, 7.2 & 8.3).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, IrStream, OutputMode, StderrMode};
use thiserror::Error;

use crate::platform::posix;
use crate::strict_modes::{self, StrictModeError};

#[derive(Debug, Error)]
pub enum HookError {
    #[error(transparent)]
    StrictMode(#[from] StrictModeError),
    #[error("hook `{hook}` failed (exit {code}): {stderr}")]
    Failed {
        hook: String,
        code: i32,
        stderr: String,
    },
    #[error("hook `{hook}` produced invalid output: {reason}")]
    InvalidOutput { hook: String, reason: String },
    #[error("hook `{hook}` I/O error: {source}")]
    Io {
        hook: String,
        #[source]
        source: io::Error,
    },
}

impl HookError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::StrictMode(e) => e.exit_code(),
            _ => ExitCode::RuntimeError,
        }
    }
}

#[derive(Debug, Clone)]
enum HookEntry {
    BuiltinPosixRuntime,
    External { path: PathBuf, dir: PathBuf },
    Masked,
}

/// Execute merged `pre-context.d/` pipeline (built-in `00-posix-runtime` + external scripts
/// under `hooks_dir/pre-context.d/`) on a Canonical IR v1 JSONL stream.
#[allow(clippy::too_many_arguments)]
pub fn run_pre_context(
    mut stream: IrStream,
    hooks_dir: &Path,
    cwd: &Path,
    output_mode: OutputMode,
    shell_enabled: bool,
    stderr_mode: StderrMode,
    tool_name: &str,
    has_inbox: bool,
) -> Result<IrStream, HookError> {
    let entries = collect_pre_context_hooks(hooks_dir);

    for (name, entry) in entries {
        match entry {
            HookEntry::Masked => continue,
            HookEntry::BuiltinPosixRuntime => {
                apply_builtin_posix_runtime(
                    &mut stream,
                    output_mode,
                    shell_enabled,
                    stderr_mode,
                    tool_name,
                    has_inbox,
                );
            }
            HookEntry::External { path, dir } => {
                strict_modes::verify_dir(&dir)?;
                strict_modes::verify_file(&path)?;
                let input = stream.to_jsonl().map_err(|e| HookError::InvalidOutput {
                    hook: name.clone(),
                    reason: e.to_string(),
                })?;
                let stdout = run_external_hook(&name, &path, &input, cwd)?;
                stream = IrStream::from_jsonl(&stdout).map_err(|e| HookError::InvalidOutput {
                    hook: name.clone(),
                    reason: e.to_string(),
                })?;
            }
        }
    }

    Ok(stream)
}

fn collect_pre_context_hooks(hooks_dir: &Path) -> BTreeMap<String, HookEntry> {
    let mut map: BTreeMap<String, HookEntry> = BTreeMap::new();
    map.insert("00-posix-runtime".into(), HookEntry::BuiltinPosixRuntime);

    let pre_dir = hooks_dir.join("pre-context.d");
    let Ok(read_dir) = fs::read_dir(&pre_dir) else {
        return map;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if strict_modes::is_hook_mask(&path) {
            map.insert(name, HookEntry::Masked);
        } else if posix::is_executable_file(&path) {
            map.insert(
                name,
                HookEntry::External {
                    path,
                    dir: pre_dir.clone(),
                },
            );
        }
    }

    map
}

/// Built-in `00-posix-runtime`: minimal Layer 1 POSIX I/O contract (RFC §4.4).
fn apply_builtin_posix_runtime(
    stream: &mut IrStream,
    output_mode: OutputMode,
    shell_enabled: bool,
    stderr_mode: StderrMode,
    tool_name: &str,
    has_inbox: bool,
) {
    let inbox_line = if has_inbox && shell_enabled {
        "\n- You are running in persistent `--inbox` mode: `$ANIX_INBOX` points to your Maildir inbox; background jobs or sub-agents can send updates back via `anix send \"$ANIX_INBOX\"`."
    } else {
        ""
    };

    let contract = match (output_mode, shell_enabled, stderr_mode) {
        (OutputMode::Reply, false, _) => "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=reply`, `shell=none`, pure-text mode).
- No shell or tools are available.
- Your response text is streamed directly to `stdout` (`fd 1`) as the final deliverable. Output only the deliverable requested by your instructions without extraneous preamble.
</posix_runtime>"
            .to_owned(),
        (OutputMode::Reply, true, StderrMode::Auto) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=reply`, `stderr=auto`, tool=`{tool_name}`).
- Your FINAL text reply (after all `{tool_name}` tool calls finish) is delivered directly to `stdout` (`fd 1`). Intermediate commentary during tool-calling turns goes to `stderr` (`fd 2`).
- `$ANIX_OUT` is NOT mounted in `output=reply` mode; do NOT write to `$ANIX_OUT`.
- You may write progress notes or warnings during shell execution to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
- `$ANIX_CTL` is a write-only JSONL side-channel pipe for runtime control frames (e.g. `compact`, `fold`, `media`, `hold`, `release`).{inbox_line}
</posix_runtime>"
        ),
        (OutputMode::Reply, true, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=reply`, `stderr=explicit`, tool=`{tool_name}`).
- Your FINAL text reply (after all `{tool_name}` tool calls finish) is delivered directly to `stdout` (`fd 1`). Intermediate text during tool-calling turns is hidden from humans.
- `$ANIX_OUT` is NOT mounted in `output=reply` mode; do NOT write to `$ANIX_OUT`.
- Any progress note or warning intended for humans during shell execution MUST be explicitly written via `{tool_name}` to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
- `$ANIX_CTL` is a write-only JSONL side-channel pipe for runtime control frames (e.g. `compact`, `fold`, `media`, `hold`, `release`).{inbox_line}
</posix_runtime>"
        ),
        (OutputMode::Explicit, _, StderrMode::Auto) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=explicit`, `stderr=auto`, tool=`{tool_name}`).
- Your normal text replies are streamed to `stderr` for human observation.
- Final deliverable payload MUST be written via `{tool_name}` to `$ANIX_OUT` (`stdout`, `fd 1`). Never rely on normal prose for structured deliverables.
- You may write progress notes or warnings during shell execution to `$ANIX_ERR` (`fd 2`).
- `$ANIX_OUT`, `$ANIX_ERR`, and `$ANIX_CTL` are write-only anonymous pipes: redirect into them (e.g. `> \"$ANIX_OUT\"`), NEVER read, `cat`, `wc`, `head`, or inspect them.{inbox_line}
</posix_runtime>"
        ),
        (OutputMode::Explicit, _, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=explicit`, `stderr=explicit`, tool=`{tool_name}`).
- Your normal text replies are internal thoughts ONLY and are COMPLETELY INVISIBLE to humans (recorded to structured Trace only when enabled).
- Any message, status, or warning intended for humans MUST be explicitly written via `{tool_name}` to `$ANIX_ERR` (`fd 2`).
- Final deliverable payload MUST be written via `{tool_name}` to `$ANIX_OUT` (`stdout`, `fd 1`).
- `$ANIX_OUT`, `$ANIX_ERR`, and `$ANIX_CTL` are write-only anonymous pipes: redirect into them (e.g. `> \"$ANIX_OUT\"`), NEVER read, `cat`, `wc`, `head`, or inspect them.{inbox_line}
</posix_runtime>"
        ),
        (OutputMode::None, false, _) => "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=none`, `shell=none`).
- `stdout` (`fd 1`) is disabled (0 bytes); no tools are available.
</posix_runtime>"
            .to_owned(),
        (OutputMode::None, true, StderrMode::Auto) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=none`, `stderr=auto`, tool=`{tool_name}`).
- `stdout` (`fd 1`) is strictly silent and `$ANIX_OUT` is NOT mounted. Deliver results via side effects (e.g. editing files) using `{tool_name}`.
- Your normal text replies and `$ANIX_ERR` writes go to `stderr` (`fd 2`) for human observation (`$ANIX_ERR` is a write-only pipe: redirect into it, never read it).
- `$ANIX_CTL` is a write-only JSONL side-channel pipe for runtime control frames.{inbox_line}
</posix_runtime>"
        ),
        (OutputMode::None, true, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=none`, `stderr=explicit`, tool=`{tool_name}`).
- `stdout` (`fd 1`) is strictly silent and `$ANIX_OUT` is NOT mounted. Deliver results via side effects (e.g. editing files) using `{tool_name}`.
- Your normal text replies are invisible to humans; write any human-facing status or warnings explicitly via `{tool_name}` to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
- `$ANIX_CTL` is a write-only JSONL side-channel pipe for runtime control frames.{inbox_line}
</posix_runtime>"
        ),
    };

    prepend_to_first_system(stream, &contract);
}

fn prepend_to_first_system(stream: &mut IrStream, prefix: &str) {
    if let Some(IrMessage::System { text, .. }) = stream
        .messages
        .iter_mut()
        .find(|m| matches!(m, IrMessage::System { .. }))
    {
        if text.is_empty() {
            *text = prefix.to_owned();
        } else {
            *text = format!("{prefix}\n\n{text}");
        }
    } else {
        stream.messages.insert(0, IrMessage::system(0, prefix));
    }
}

fn run_external_hook(
    name: &str,
    path: &Path,
    stdin_data: &str,
    cwd: &Path,
) -> Result<String, HookError> {
    let mut cmd = Command::new(path);
    cmd.current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|source| HookError::Io {
        hook: name.to_owned(),
        source,
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let data = stdin_data.as_bytes().to_vec();
        thread::spawn(move || {
            let _ = stdin.write_all(&data);
        });
    }

    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    let out_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(ref mut p) = stdout_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });
    let err_handle = thread::spawn(move || {
        let mut s = String::new();
        if let Some(ref mut p) = stderr_pipe {
            let _ = p.read_to_string(&mut s);
        }
        s
    });

    let status = child.wait().map_err(|source| HookError::Io {
        hook: name.to_owned(),
        source,
    })?;

    let stdout = out_handle.join().unwrap_or_default();
    let stderr = err_handle.join().unwrap_or_default();

    if !status.success() {
        return Err(HookError::Failed {
            hook: name.to_owned(),
            code: status.code().unwrap_or(1),
            stderr: stderr.trim().to_owned(),
        });
    }

    Ok(stdout)
}
