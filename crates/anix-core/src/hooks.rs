//! Built-in hooks (`00-posix-runtime`, `20-skills`, `30-tips`) and systemd-style `run-parts`
//! lifecycle hooks (`pre-context.d/`, `post-tool.d/`, `on-exit.d/`) (Sections 4.4, 8.3, 9.3, 9.4, 9.6).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anix_proto::exit_code::ExitCode;
use anix_proto::hooks::{OnExitPayload, PostToolPayload};
use anix_proto::ir::{IrMessage, IrStream, OutputMode, StderrMode};
use serde_json::{Map, Value};
use serde_yaml_ng::Value as YamlValue;
use thiserror::Error;

use crate::config::AgentPaths;
use crate::platform::{self, ChildWaitOutcome, posix};
use crate::trace::Tracer;
use crate::trust::{self, TrustError};

const ON_EXIT_TIMEOUT: Duration = Duration::from_secs(5);

const BUILTIN_TIPS: &[(&str, &str)] = &[
    (
        "how-to-background.md",
        include_str!("../tips/how-to-background.md"),
    ),
    ("how-to-goal.md", include_str!("../tips/how-to-goal.md")),
    ("how-to-loop.md", include_str!("../tips/how-to-loop.md")),
    ("how-to-mcp.md", include_str!("../tips/how-to-mcp.md")),
    (
        "how-to-sub-agent.md",
        include_str!("../tips/how-to-sub-agent.md"),
    ),
];

#[derive(Debug, Error)]
pub enum HookError {
    #[error(transparent)]
    Trust(#[from] TrustError),
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
            Self::Trust(_) => ExitCode::TempFail,
            _ => ExitCode::RuntimeError,
        }
    }
}

#[derive(Debug, Clone)]
enum HookEntry {
    BuiltinPosixRuntime,
    BuiltinSkills,
    BuiltinTips,
    External { path: PathBuf, from_project: bool },
    Masked,
}

/// Execute merged `pre-context.d/` pipeline (including built-ins `00-posix-runtime`, `20-skills`,
/// & `30-tips`) on a Canonical IR v1 JSONL stream.
pub fn run_pre_context(
    mut stream: IrStream,
    paths: &AgentPaths,
    output_mode: OutputMode,
    shell_enabled: bool,
    stderr_mode: StderrMode,
    vision_enabled: bool,
    has_vision_bridge: bool,
) -> Result<IrStream, HookError> {
    let entries = collect_stage_hooks("pre-context.d", paths, true);

    for (name, entry) in entries {
        match entry {
            HookEntry::Masked => continue,
            HookEntry::BuiltinPosixRuntime => {
                apply_builtin_posix_runtime(
                    &mut stream,
                    output_mode,
                    shell_enabled,
                    stderr_mode,
                    vision_enabled,
                    has_vision_bridge,
                );
            }
            HookEntry::BuiltinSkills => {
                if shell_enabled {
                    apply_builtin_skills(&mut stream, paths);
                }
            }
            HookEntry::BuiltinTips => {
                if shell_enabled {
                    apply_builtin_tips(&mut stream, paths);
                }
            }
            HookEntry::External { path, from_project } => {
                if from_project {
                    trust::verify_trusted(&path, paths)?;
                }
                let input = stream.to_jsonl().map_err(|e| HookError::InvalidOutput {
                    hook: name.clone(),
                    reason: e.to_string(),
                })?;
                let stdout = run_external_hook(&name, &path, &input, &paths.cwd, None)?;
                stream = IrStream::from_jsonl(&stdout).map_err(|e| HookError::InvalidOutput {
                    hook: name.clone(),
                    reason: e.to_string(),
                })?;
            }
        }
    }

    Ok(stream)
}

/// Execute merged `post-tool.d/` hooks on a [`PostToolPayload`].
pub fn run_post_tool(
    mut payload: PostToolPayload,
    paths: &AgentPaths,
) -> Result<PostToolPayload, HookError> {
    let entries = collect_stage_hooks("post-tool.d", paths, false);

    for (name, entry) in entries {
        if let HookEntry::External { path, from_project } = entry {
            if from_project {
                trust::verify_trusted(&path, paths)?;
            }
            let input = serde_json::to_string(&payload).map_err(|e| HookError::InvalidOutput {
                hook: name.clone(),
                reason: e.to_string(),
            })?;
            let stdout = run_external_hook(&name, &path, &input, &paths.cwd, None)?;
            payload = serde_json::from_str(&stdout).map_err(|e| HookError::InvalidOutput {
                hook: name.clone(),
                reason: e.to_string(),
            })?;
        }
    }

    Ok(payload)
}

/// Execute merged `on-exit.d/` hooks (5s timeout per hook; failures logged to Trace ONLY,
/// never overriding the main exit code — Section 9.3).
pub fn run_on_exit(payload: &OnExitPayload, paths: &AgentPaths, tracer: &mut Tracer) {
    let entries = collect_stage_hooks("on-exit.d", paths, false);
    let Ok(input) = serde_json::to_string(payload) else {
        return;
    };

    for (name, entry) in entries {
        if let HookEntry::External { path, from_project } = entry {
            if from_project && let Err(err) = trust::verify_trusted(&path, paths) {
                let mut fields = Map::new();
                fields.insert("hook".into(), Value::String(name));
                fields.insert("message".into(), Value::String(err.to_string()));
                tracer.emit_event("warning", fields);
                continue;
            }
            if let Err(err) =
                run_external_hook(&name, &path, &input, &paths.cwd, Some(ON_EXIT_TIMEOUT))
            {
                let mut fields = Map::new();
                fields.insert("hook".into(), Value::String(name));
                fields.insert("message".into(), Value::String(err.to_string()));
                tracer.emit_event("warning", fields);
            }
        }
    }
}

/// Collect and merge hooks across built-ins, global config, and project `.anix/hooks/`
/// in deterministic ASCII lexicographical order (Section 8.3).
fn collect_stage_hooks(
    stage: &str,
    paths: &AgentPaths,
    include_pre_context_builtins: bool,
) -> BTreeMap<String, HookEntry> {
    let mut map: BTreeMap<String, HookEntry> = BTreeMap::new();

    // 1. Lowest priority: built-in hooks compiled into `anix`.
    if include_pre_context_builtins {
        map.insert("00-posix-runtime".into(), HookEntry::BuiltinPosixRuntime);
        map.insert("20-skills".into(), HookEntry::BuiltinSkills);
        map.insert("30-tips".into(), HookEntry::BuiltinTips);
    }

    // 2. Global `$XDG_CONFIG_HOME/anix/hooks/<stage>.d/`
    let global_dir = paths.global_config_dir.join("hooks").join(stage);
    scan_hook_dir(&global_dir, false, &mut map);

    // 3. Highest priority: project `.anix/hooks/<stage>.d/`
    if let Some(ref proj_agent) = paths.project_agent_dir {
        let project_dir = proj_agent.join("hooks").join(stage);
        scan_hook_dir(&project_dir, true, &mut map);
    }

    map
}

fn scan_hook_dir(dir: &Path, from_project: bool, map: &mut BTreeMap<String, HookEntry>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if trust::is_hook_mask(&path) {
            map.insert(name, HookEntry::Masked);
        } else if posix::is_executable_file(&path) {
            map.insert(name, HookEntry::External { path, from_project });
        }
    }
}

/// Built-in `00-posix-runtime`: deterministically inject POSIX I/O, output contract, and vision contract (v1.2.0 §4.4).
fn apply_builtin_posix_runtime(
    stream: &mut IrStream,
    output_mode: OutputMode,
    shell_enabled: bool,
    stderr_mode: StderrMode,
    vision_enabled: bool,
    has_vision_bridge: bool,
) {
    let vision_line = if vision_enabled {
        "- Use `aview [--max-dim 1568] [--pages 1-3] <file>` inside `msh` to inspect images/PDFs visually."
    } else if has_vision_bridge {
        "- Current model has `vision=false`, backed by a text vision bridge: calling `aview <file>` translates images/PDFs into `<vision_description>` text."
    } else {
        "- Current model does NOT support native vision (`vision=false`). Do NOT use `aview` for native image viewing; use text tools (e.g. OCR, `pdftotext`) or delegate to a vision-capable sub-agent (e.g. `vision.agent`)."
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
You are running inside the Anix POSIX runtime (`output=reply`, `stderr=auto`).
- Your FINAL text reply (after all `msh` tool calls finish) is delivered directly to `stdout` (`fd 1`). Intermediate commentary during tool-calling turns goes to `stderr` (`fd 2`).
- `$ANIX_OUT` is NOT mounted in `output=reply` mode; do NOT write to `$ANIX_OUT`.
- You may write progress notes or warnings during shell execution to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
</posix_runtime>"
        ),
        (OutputMode::Reply, true, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=reply`, `stderr=explicit`).
- Your FINAL text reply (after all `msh` tool calls finish) is delivered directly to `stdout` (`fd 1`). Intermediate text during tool-calling turns is hidden from humans.
- `$ANIX_OUT` is NOT mounted in `output=reply` mode; do NOT write to `$ANIX_OUT`.
- Any progress note or warning intended for humans during shell execution MUST be explicitly written via `msh` to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
</posix_runtime>"
        ),
        (OutputMode::Explicit, _, StderrMode::Auto) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=explicit`, `stderr=auto`).
- Your normal text replies are streamed to `stderr` for human observation.
- Final deliverable payload MUST be written via the `msh` tool to `$ANIX_OUT` (`stdout`, `fd 1`). Never rely on normal prose for structured deliverables.
- You may write progress notes or warnings during shell execution to `$ANIX_ERR` (`fd 2`).
- `$ANIX_OUT` and `$ANIX_ERR` are write-only anonymous pipes: redirect into them (e.g. `> \"$ANIX_OUT\"`), NEVER read, `cat`, `wc`, `head`, or inspect them (reading fails with Permission denied or blocks until timeout).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
</posix_runtime>"
        ),
        (OutputMode::Explicit, _, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=explicit`, `stderr=explicit`).
- Your normal text replies are internal thoughts ONLY and are COMPLETELY INVISIBLE to humans (recorded to structured Trace only when enabled).
- Any message, status, or warning intended for humans MUST be explicitly written via `msh` to `$ANIX_ERR` (`fd 2`).
- Final deliverable payload MUST be written via `msh` to `$ANIX_OUT` (`stdout`, `fd 1`).
- `$ANIX_OUT` and `$ANIX_ERR` are write-only anonymous pipes: redirect into them (e.g. `> \"$ANIX_OUT\"`), NEVER read, `cat`, `wc`, `head`, or inspect them (reading fails with Permission denied or blocks until timeout).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
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
You are running inside the Anix POSIX runtime (`output=none`, `stderr=auto`).
- `stdout` (`fd 1`) is strictly silent and `$ANIX_OUT` is NOT mounted. Deliver results via side effects (e.g. editing files) using `msh`.
- Your normal text replies and `$ANIX_ERR` writes go to `stderr` (`fd 2`) for human observation (`$ANIX_ERR` is a write-only pipe: redirect into it, never read it).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
</posix_runtime>"
        ),
        (OutputMode::None, true, StderrMode::Explicit) => format!(
            "\
<posix_runtime>
You are running inside the Anix POSIX runtime (`output=none`, `stderr=explicit`).
- `stdout` (`fd 1`) is strictly silent and `$ANIX_OUT` is NOT mounted. Deliver results via side effects (e.g. editing files) using `msh`.
- Your normal text replies are invisible to humans; write any human-facing status or warnings explicitly via `msh` to `$ANIX_ERR` (`fd 2`, write-only pipe: redirect into it, never read it).
{vision_line}
- Use `aclr -t <turn_id> [\"<summary>\"]` to fold dead-end tail turns while preserving KV-cache prefix (omitting summary auto-summarizes), or `aclr -s` for full compaction.
</posix_runtime>"
        ),
    };

    prepend_to_first_system(stream, &contract);
}

/// Built-in `20-skills`: scan 6 skill directories in exact priority order and append
/// deterministic `<available_skills>` XML to the first `role: "system"` message (Section 9.4).
fn apply_builtin_skills(stream: &mut IrStream, paths: &AgentPaths) {
    // Priority order (high overrides low by skill `name`):
    // ./.anix/skills/ -> ./.skills/ -> ./.claude/skills/ ->
    // ~/.config/anix/skills/ -> ~/.skills/ -> ~/.claude/skills/
    let search_dirs: Vec<PathBuf> = vec![
        paths.cwd.join(".anix/skills"),
        paths.cwd.join(".skills"),
        paths.cwd.join(".claude/skills"),
        paths.global_config_dir.join("skills"),
        paths.home_dir.join(".skills"),
        paths.home_dir.join(".claude/skills"),
    ];

    let mut skills: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();

    for dir in search_dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        children.sort();
        for skill_dir in children {
            if !skill_dir.is_dir() {
                continue;
            }
            let skill_md = skill_dir.join("SKILL.md");
            if !skill_md.is_file() {
                continue;
            }
            if let Some((name, desc)) = parse_skill_frontmatter(&skill_md) {
                let abs_path = fs::canonicalize(&skill_md).unwrap_or(skill_md);
                // First occurrence wins (higher priority dir).
                skills.entry(name).or_insert((abs_path, desc));
            }
        }
    }

    if skills.is_empty() {
        return;
    }

    let mut xml = String::from("<available_skills>\n");
    for (name, (path, desc)) in skills {
        xml.push_str(&format!(
            "  <skill name=\"{}\" path=\"{}\">{}</skill>\n",
            xml_escape(&name),
            xml_escape(&path.display().to_string()),
            xml_escape(&desc)
        ));
    }
    xml.push_str("</available_skills>");

    append_to_first_system(stream, &xml);
}

/// Built-in `30-tips`: merge built-in default guides (`$XDG_STATE_HOME/anix/builtin-tips/`),
/// global tips (`~/.config/anix/tips/`), and project tips (`<cwd>/.anix/tips/`) in ascending
/// priority order (higher overrides lower; 0-byte file or `/dev/null` symlink masks a single tip),
/// and append canonical absolute paths sorted in ASCII order inside `<runtime_tips>` (Section 9.6).
fn apply_builtin_tips(stream: &mut IrStream, paths: &AgentPaths) {
    let mut tips: BTreeMap<String, PathBuf> = BTreeMap::new();

    // 1. Lowest priority: compiled-in defaults materialized atomically for `msh` read access.
    if let Some(builtin_dir) = materialize_builtin_tips(paths) {
        for &(name, _) in BUILTIN_TIPS {
            let path = builtin_dir.join(name);
            if path.is_file() {
                let abs_path = fs::canonicalize(&path).unwrap_or(path);
                tips.insert(name.to_owned(), abs_path);
            }
        }
    }

    // 2. Global (`~/.config/anix/tips/`) -> 3. Project (`<cwd>/.anix/tips/`)
    let override_dirs = [
        paths.global_config_dir.join("tips"),
        paths.cwd.join(".anix/tips"),
    ];

    for dir in &override_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Ignore hidden files and non-`.md` files.
            if name.starts_with('.') || !name.ends_with(".md") {
                continue;
            }
            let path = entry.path();
            // 0-byte regular file or `/dev/null` symlink masks this tip (including built-ins).
            if trust::is_hook_mask(&path) {
                tips.remove(&name);
                continue;
            }
            // Include regular files (ignoring subdirectories).
            if !path.is_file() {
                continue;
            }
            let abs_path = fs::canonicalize(&path).unwrap_or(path);
            tips.insert(name, abs_path);
        }
    }

    if tips.is_empty() {
        return;
    }

    let mut xml = String::from(
        "<runtime_tips>\nFor advanced POSIX workflows, read the relevant guide via msh when needed:\n",
    );
    for path in tips.values() {
        xml.push_str(&path.display().to_string());
        xml.push('\n');
    }
    xml.push_str("</runtime_tips>");

    append_to_first_system(stream, &xml);
}

fn materialize_builtin_tips(paths: &AgentPaths) -> Option<PathBuf> {
    let candidates = [
        paths.state_dir.join("builtin-tips"),
        paths.runtime_dir.join("builtin-tips"),
    ];
    for dir in candidates {
        if fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let mut all_ok = true;
        for &(name, content) in BUILTIN_TIPS {
            let dest = dir.join(name);
            let up_to_date = fs::read_to_string(&dest)
                .map(|existing| existing == content)
                .unwrap_or(false);
            if up_to_date {
                continue;
            }
            if posix::write_atomic_0600(&dest, content.as_bytes()).is_err() {
                all_ok = false;
                break;
            }
        }
        if all_ok {
            return Some(dir);
        }
    }
    None
}

fn parse_skill_frontmatter(path: &Path) -> Option<(String, String)> {
    let raw = fs::read_to_string(path).ok()?;
    let normalized = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if lines.first().copied() != Some("---") {
        return None;
    }
    let end_idx = lines.iter().skip(1).position(|&l| l == "---")?;
    let yaml = lines[1..=end_idx].join("\n");
    let doc: YamlValue = serde_yaml_ng::from_str(&yaml).ok()?;
    let map = doc.as_mapping()?;
    let name = map.get("name")?.as_str()?.trim().to_owned();
    let desc = map.get("description")?.as_str()?.trim().to_owned();
    if name.is_empty() || desc.is_empty() {
        return None;
    }
    Some((name, desc))
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

fn append_to_first_system(stream: &mut IrStream, suffix: &str) {
    if let Some(IrMessage::System { text, .. }) = stream
        .messages
        .iter_mut()
        .find(|m| matches!(m, IrMessage::System { .. }))
    {
        if text.is_empty() {
            *text = suffix.to_owned();
        } else {
            text.push_str("\n\n");
            text.push_str(suffix);
        }
    } else {
        stream.messages.insert(0, IrMessage::system(0, suffix));
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn run_external_hook(
    name: &str,
    path: &Path,
    stdin_data: &str,
    cwd: &Path,
    timeout: Option<Duration>,
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

    let status = if let Some(limit) = timeout {
        let deadline = Instant::now() + limit;
        match platform::wait_child_until(&mut child, Some(deadline), None).map_err(|source| {
            HookError::Io {
                hook: name.to_owned(),
                source,
            }
        })? {
            ChildWaitOutcome::Exited(st) => st,
            ChildWaitOutcome::TimedOut | ChildWaitOutcome::SignalWakeup => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(HookError::Failed {
                    hook: name.to_owned(),
                    code: ExitCode::TimeoutOrBudget.as_i32(),
                    stderr: format!("hook timed out after {}s", limit.as_secs()),
                });
            }
        }
    } else {
        child.wait().map_err(|source| HookError::Io {
            hook: name.to_owned(),
            source,
        })?
    };

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
