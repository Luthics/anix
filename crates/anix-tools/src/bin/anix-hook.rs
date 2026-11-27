//! `anix-hook` — Layer 2 standalone `pre-context.d/` hook dispatcher
//! (`10-agents-md`, `15-memory`, `20-skills`, & `30-tips`).
//!
//! Invoked via BusyBox-style symlink name (`10-agents-md` / `15-memory` / `20-skills` / `30-tips`)
//! or explicit flag (`anix-hook --agents-md` / `--memory` / `--skills` / `--tips`). Reads
//! Canonical IR v1 JSONL from `stdin` and writes updated Canonical IR v1 JSONL to `stdout`.

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode as StdExitCode;

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, IrStream};
use serde_yaml_ng::Value as YamlValue;
use uuid::Uuid;

const AGENTS_MD_MAX_FILE_BYTES: usize = 16 * 1024;
const AGENTS_MD_MAX_TOTAL_BYTES: usize = 48 * 1024;
const MEMORY_INDEX_MAX_LINES: usize = 200;
const MEMORY_INDEX_MAX_BYTES: usize = 25 * 1024;

const BUILTIN_TIPS: &[(&str, &str, &str)] = &[
    (
        "how-to-background.md",
        "`msh` background jobs (`timeout_sec: 0`), `hold` leases, `$ANIX_INBOX` wakeup",
        include_str!("../../../../runtime/tips/how-to-background.md"),
    ),
    (
        "how-to-goal.md",
        "goal-driven verification loops with bounded retry budgets",
        include_str!("../../../../runtime/tips/how-to-goal.md"),
    ),
    (
        "how-to-loop.md",
        "persistent `--inbox` daemons and `anix send --wait` RPC",
        include_str!("../../../../runtime/tips/how-to-loop.md"),
    ),
    (
        "how-to-mcp.md",
        "discovering & calling MCP servers (`mcporter` / `curl` / stdio JSON-RPC)",
        include_str!("../../../../runtime/tips/how-to-mcp.md"),
    ),
    (
        "how-to-runtime.md",
        "POSIX FDs, `msh` folding, `aview` (images/PDF), `aclr` (fold/compact), `aplan` (live checklist), Tree WAL",
        include_str!("../../../../runtime/tips/how-to-runtime.md"),
    ),
    (
        "how-to-sub-agent.md",
        "self-forking via `anix fork \"$ANIX_WAL\" --leaf \"$ANIX_LEAF\"`, parallel sub-agents & `--notify`",
        include_str!("../../../../runtime/tips/how-to-sub-agent.md"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookMode {
    AgentsMd,
    Memory,
    Skills,
    Tips,
}

fn detect_mode() -> Option<HookMode> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("anix-hook {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }
    if let Some(arg1) = args.get(1) {
        match arg1.as_str() {
            "--agents-md" | "agents-md" => return Some(HookMode::AgentsMd),
            "--memory" | "memory" => return Some(HookMode::Memory),
            "--skills" | "skills" => return Some(HookMode::Skills),
            "--tips" | "tips" => return Some(HookMode::Tips),
            _ => {}
        }
    }
    let argv0 = args.first().map(String::as_str).unwrap_or("");
    let base = Path::new(argv0)
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if base.contains("agents") {
        Some(HookMode::AgentsMd)
    } else if base.contains("memory") {
        Some(HookMode::Memory)
    } else if base.contains("skills") {
        Some(HookMode::Skills)
    } else if base.contains("tips") {
        Some(HookMode::Tips)
    } else {
        None
    }
}

fn main() -> StdExitCode {
    let Some(mode) = detect_mode() else {
        eprintln!(
            "Usage: anix-hook --agents-md | --memory | --skills | --tips (or symlink as 10-agents-md / 15-memory / 20-skills / 30-tips)"
        );
        return ExitCode::UsageError.into();
    };

    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("anix-hook: failed reading stdin: {err}");
        return ExitCode::RuntimeError.into();
    }

    let mut stream = match IrStream::from_jsonl(&input) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix-hook: invalid Canonical IR input: {err}");
            return ExitCode::RuntimeError.into();
        }
    };

    // Only inject skills / runtime tips when shell tools are enabled.
    if !stream.header.tools.is_empty() {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from(&stream.header.meta.cwd));
        let home_dir = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let config_dir = env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".config"))
            .join("anix");
        let state_dir = env::var_os("XDG_STATE_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".local/state"))
            .join("anix");
        let data_dir = env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir.join(".local/share"))
            .join("anix");
        let runtime_dir = match env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            Some(dir) => PathBuf::from(dir).join("anix"),
            None => {
                let uid = unsafe { libc::geteuid() };
                PathBuf::from(format!("/tmp/anix-{uid}"))
            }
        };

        match mode {
            HookMode::AgentsMd => apply_agents_md(&mut stream, &cwd, &config_dir),
            HookMode::Memory => apply_memory(&mut stream, &cwd, &data_dir),
            HookMode::Skills => apply_skills(&mut stream, &cwd, &home_dir, &config_dir),
            HookMode::Tips => {
                apply_tips(&mut stream, &cwd, &config_dir, &state_dir, &runtime_dir);
            }
        }
    }

    let output = match stream.to_jsonl() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix-hook: failed serializing Canonical IR: {err}");
            return ExitCode::RuntimeError.into();
        }
    };
    let mut stdout = io::stdout().lock();
    if let Err(err) = stdout.write_all(output.as_bytes()) {
        eprintln!("anix-hook: failed writing stdout: {err}");
        return ExitCode::RuntimeError.into();
    }
    let _ = stdout.flush();
    ExitCode::Success.into()
}

fn agents_md_disabled_by_env() -> bool {
    env::var("ANIX_AGENTS_MD")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(false)
}

/// Find nearest enclosing git root without spawning a subprocess (supports both `.git` dirs and
/// worktree `.git` files).
fn find_git_root(cwd: &Path) -> Option<PathBuf> {
    let mut cur = Some(cwd);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

/// Collect candidate search directories in broadest-to-most-specific order:
/// global `config_dir` (`~/.config/anix`) followed by `git_root` -> ... -> `cwd`.
fn instruction_search_dirs(cwd: &Path, config_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen_dirs = HashSet::new();

    let mut push_dir = |p: PathBuf| {
        let key = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
        if seen_dirs.insert(key) {
            dirs.push(p);
        }
    };

    if config_dir.is_dir() {
        push_dir(config_dir.to_path_buf());
    }

    let cwd_canon = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    if let Some(root) = find_git_root(&cwd_canon) {
        let mut chain = Vec::new();
        let mut cur = Some(cwd_canon.as_path());
        while let Some(d) = cur {
            chain.push(d.to_path_buf());
            if d == root {
                break;
            }
            cur = d.parent();
        }
        chain.reverse();
        for d in chain {
            push_dir(d);
        }
    } else {
        push_dir(cwd_canon);
    }

    dirs
}

fn truncate_utf8_bytes(s: &str, max_bytes: usize) -> (&str, bool) {
    if s.len() <= max_bytes {
        return (s, false);
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    (&s[..end], true)
}

fn apply_agents_md(stream: &mut IrStream, cwd: &Path, config_dir: &Path) {
    if agents_md_disabled_by_env() {
        return;
    }
    let candidates: &[(&str, &str)] = &[
        ("AGENTS.md", "agents_md"),
        ("AGENTS.local.md", "agents_md"),
        ("CLAUDE.md", "claude_md"),
        ("CLAUDE.local.md", "claude_md"),
    ];

    let search_dirs = instruction_search_dirs(cwd, config_dir);
    let mut seen_paths: HashSet<PathBuf> = HashSet::new();
    let mut seen_contents: HashSet<String> = HashSet::new();
    let mut blocks: Vec<String> = Vec::new();
    let mut total_bytes = 0usize;

    for dir in search_dirs {
        for &(filename, tag) in candidates {
            let path = dir.join(filename);
            if !path.is_file() {
                continue;
            }
            let canon = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if !seen_paths.insert(canon.clone()) {
                continue;
            }
            // Fail-open on unreadable or binary files so dirty workspace files never abort `anix`.
            let Ok(raw_bytes) = fs::read(&canon) else {
                continue;
            };
            if raw_bytes.is_empty() || raw_bytes.iter().take(4096).any(|&b| b == 0) {
                continue;
            }
            let Ok(raw_text) = String::from_utf8(raw_bytes) else {
                continue;
            };
            let trimmed = raw_text.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !seen_contents.insert(trimmed.to_owned()) {
                continue;
            }

            let path_attr = xml_escape(&canon.display().to_string());
            let remaining = AGENTS_MD_MAX_TOTAL_BYTES.saturating_sub(total_bytes);
            if remaining == 0 {
                blocks.push(format!(
                    "<{tag} path=\"{path_attr}\" truncated=\"true\">(omitted due to total size cap; read full file at {path_attr})</{tag}>"
                ));
                continue;
            }

            let limit = AGENTS_MD_MAX_FILE_BYTES.min(remaining);
            let (slice, was_truncated) = truncate_utf8_bytes(trimmed, limit);
            total_bytes = total_bytes.saturating_add(slice.len());

            if was_truncated {
                blocks.push(format!(
                    "<{tag} path=\"{path_attr}\" truncated=\"true\">\n{slice}\n... (truncated; read full file at {path_attr})\n</{tag}>"
                ));
            } else {
                blocks.push(format!("<{tag} path=\"{path_attr}\">\n{slice}\n</{tag}>"));
            }
        }
    }

    if blocks.is_empty() {
        return;
    }

    let xml = format!(
        "<project_instructions>\n{}\n</project_instructions>",
        blocks.join("\n")
    );
    append_to_first_system(stream, &xml);
}

fn memory_disabled_by_env() -> bool {
    env::var("ANIX_MEMORY")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        })
        .unwrap_or(false)
}

/// Resolve the canonical project root shared across git worktrees without spawning subprocesses.
fn resolve_shared_project_root(cwd: &Path) -> PathBuf {
    let cwd_canon = fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let mut cur = Some(cwd_canon.as_path());
    while let Some(dir) = cur {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return dir.to_path_buf();
        }
        if dot_git.is_file() {
            if let Ok(raw) = fs::read_to_string(&dot_git)
                && let Some(gd_raw) = raw.lines().find_map(|l| l.trim().strip_prefix("gitdir:"))
            {
                let gd_trimmed = gd_raw.trim();
                let gd_path = PathBuf::from(gd_trimmed);
                let gitdir = if gd_path.is_absolute() {
                    gd_path
                } else {
                    dir.join(gd_path)
                };
                let gitdir_canon = fs::canonicalize(&gitdir).unwrap_or(gitdir);
                if let Ok(common_raw) = fs::read_to_string(gitdir_canon.join("commondir")) {
                    let cd_trimmed = common_raw.trim();
                    if !cd_trimmed.is_empty() {
                        let common_path = if Path::new(cd_trimmed).is_absolute() {
                            PathBuf::from(cd_trimmed)
                        } else {
                            gitdir_canon.join(cd_trimmed)
                        };
                        let common_canon = fs::canonicalize(&common_path).unwrap_or(common_path);
                        if common_canon.file_name().and_then(|s| s.to_str()) == Some(".git")
                            && let Some(main_root) = common_canon.parent()
                        {
                            return main_root.to_path_buf();
                        }
                    }
                }
            }
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    cwd_canon
}

/// Stable, human-readable, collision-resistant directory key shared with Layer 3 `/memory`:
/// `<sanitized-basename>-<fnv1a32-hex>`.
fn project_memory_key(cwd: &Path) -> String {
    let root = resolve_shared_project_root(cwd);
    let canon = fs::canonicalize(&root).unwrap_or(root);
    let raw_base = canon
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("project");
    let mut clean_base: String = raw_base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    clean_base = clean_base.trim_matches('-').to_owned();
    if clean_base.is_empty() {
        clean_base = "project".to_owned();
    }
    if clean_base.len() > 32 {
        clean_base.truncate(32);
    }

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in canon.to_string_lossy().as_bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let folded = (hash ^ (hash >> 32)) as u32;
    format!("{clean_base}-{folded:08x}")
}

fn cap_memory_index(raw: &str) -> (String, bool) {
    let mut out = String::new();
    let mut truncated = false;
    for (idx, line) in raw.lines().enumerate() {
        if idx >= MEMORY_INDEX_MAX_LINES {
            truncated = true;
            break;
        }
        let add_len = line.len() + usize::from(idx > 0);
        if out.len() + add_len > MEMORY_INDEX_MAX_BYTES {
            let remaining = MEMORY_INDEX_MAX_BYTES.saturating_sub(out.len());
            if remaining > 1 {
                if idx > 0 {
                    out.push('\n');
                }
                let (part, _) = truncate_utf8_bytes(line, remaining.saturating_sub(1));
                out.push_str(part);
            }
            truncated = true;
            break;
        }
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    (out, truncated)
}

fn apply_memory(stream: &mut IrStream, cwd: &Path, data_dir: &Path) {
    if memory_disabled_by_env() {
        return;
    }
    let key = project_memory_key(cwd);
    let mem_dir = data_dir.join("memory").join(key);
    let index_path = mem_dir.join("MEMORY.md");
    let mem_dir_attr = xml_escape(&mem_dir.display().to_string());
    let index_attr = xml_escape(&index_path.display().to_string());

    let mut xml = format!(
        "<auto_memory dir=\"{mem_dir_attr}\">\n\
Persistent cross-session project memory lives in the local directory above (shared across git worktrees, outside git).\n\
- Keep `MEMORY.md` as a concise index (1 line per topic linking to `<topic>.md`; first 200 lines / 25 KiB loaded on boot).\n\
- Put detailed notes in separate `<topic>.md` files with YAML frontmatter (`name`, `description`, `type: user|feedback|project|reference`).\n\
- Record only durable, high-signal facts: user preferences, explicit corrections, non-obvious project pitfalls, or external references.\n\
- Do NOT record facts easily derived from code, git history, or ephemeral debugging state."
    );

    if index_path.is_file()
        && let Ok(bytes) = fs::read(&index_path)
        && !bytes.is_empty()
        && !bytes.iter().take(4096).any(|&b| b == 0)
        && let Ok(raw_text) = String::from_utf8(bytes)
    {
        let trimmed = raw_text.trim();
        if !trimmed.is_empty() {
            let (capped, truncated) = cap_memory_index(trimmed);
            if truncated {
                xml.push_str(&format!(
                    "\n<memory_index path=\"{index_attr}\" truncated=\"true\">\n{capped}\n... (truncated at 200 lines / 25 KiB; read or consolidate `{index_attr}`)\n</memory_index>"
                ));
            } else {
                xml.push_str(&format!(
                    "\n<memory_index path=\"{index_attr}\">\n{capped}\n</memory_index>"
                ));
            }
        }
    }

    xml.push_str("\n</auto_memory>");
    append_to_first_system(stream, &xml);
}

fn apply_skills(stream: &mut IrStream, cwd: &Path, home_dir: &Path, config_dir: &Path) {
    let search_dirs: Vec<PathBuf> = vec![
        cwd.join(".anix/skills"),
        cwd.join(".agents/skills"),
        cwd.join(".skills"),
        cwd.join(".claude/skills"),
        config_dir.join("skills"),
        home_dir.join(".agents/skills"),
        home_dir.join(".skills"),
        home_dir.join(".claude/skills"),
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

fn apply_tips(
    stream: &mut IrStream,
    cwd: &Path,
    config_dir: &Path,
    state_dir: &Path,
    runtime_dir: &Path,
) {
    let mut tips: BTreeMap<String, (PathBuf, String)> = BTreeMap::new();

    if let Some(builtin_dir) = materialize_builtin_tips(state_dir, runtime_dir) {
        for &(name, summary, _) in BUILTIN_TIPS {
            let path = builtin_dir.join(name);
            if path.is_file() {
                let abs_path = fs::canonicalize(&path).unwrap_or(path);
                tips.insert(name.to_owned(), (abs_path, summary.to_owned()));
            }
        }
    }

    let override_dirs = [config_dir.join("tips"), cwd.join(".anix/tips")];
    for dir in &override_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !name.ends_with(".md") {
                continue;
            }
            let path = entry.path();
            if is_mask(&path) {
                tips.remove(&name);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let abs_path = fs::canonicalize(&path).unwrap_or(path);
            let summary = extract_tip_summary(&abs_path)
                .or_else(|| tips.get(&name).map(|(_, s)| s.clone()))
                .unwrap_or_else(|| "custom runtime workflow guide".to_owned());
            tips.insert(name, (abs_path, summary));
        }
    }

    if tips.is_empty() {
        return;
    }

    let mut xml = String::from(
        "<runtime_tips>\nFor advanced POSIX workflows, read the relevant guide via shell when needed:\n",
    );
    for (path, summary) in tips.values() {
        xml.push_str(&format!("- {} — {}\n", path.display(), summary));
    }
    xml.push_str("</runtime_tips>");
    append_to_first_system(stream, &xml);
}

fn extract_tip_summary(path: &Path) -> Option<String> {
    let raw = fs::read_to_string(path).ok()?;
    for line in raw.lines().take(20) {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("# ") {
            let clean = heading.trim();
            if !clean.is_empty() {
                return Some(clean.to_owned());
            }
        }
    }
    None
}

fn materialize_builtin_tips(state_dir: &Path, runtime_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        state_dir.join("builtin-tips"),
        runtime_dir.join("builtin-tips"),
    ];
    for dir in candidates {
        if fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let mut all_ok = true;
        for &(name, _, content) in BUILTIN_TIPS {
            let dest = dir.join(name);
            let up_to_date = fs::read_to_string(&dest)
                .map(|existing| existing == content)
                .unwrap_or(false);
            if up_to_date {
                continue;
            }
            if write_atomic_0600(&dest, content.as_bytes()).is_err() {
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

fn write_atomic_0600(dest: &Path, content: &[u8]) -> io::Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("/tmp"));
    let tmp = parent.join(format!(".tmp_{}", Uuid::new_v4()));
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(content)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, dest).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

fn is_mask(path: &Path) -> bool {
    if let Ok(target) = fs::read_link(path)
        && target == Path::new("/dev/null")
    {
        return true;
    }
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() == 0)
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use anix_proto::ir::{HeaderTag, IrHeader, IrMeta, OutputMode, StderrMode, ToolDef};

    fn make_stream(with_tool: bool) -> IrStream {
        let mut tools = Vec::new();
        if with_tool {
            tools.push(ToolDef {
                name: "shell".into(),
                description: "run shell command".into(),
                parameters: serde_json::json!({"type": "object"}),
            });
        }
        let header = IrHeader {
            ir_version: anix_proto::IR_VERSION,
            record_type: HeaderTag::Header,
            tools,
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                stderr_mode: StderrMode::Auto,
                output_mode: Some(OutputMode::Reply),
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        };
        IrStream {
            header,
            messages: vec![IrMessage::system(0, "base system prompt")],
        }
    }

    #[test]
    fn agents_md_loads_both_agents_and_claude_blocks_with_dedup_and_truncation() {
        let root = std::env::temp_dir().join(format!("anix-hook-test-{}", Uuid::new_v4().simple()));
        let config_dir = root.join("cfg");
        let repo = root.join("repo");
        let sub = repo.join("crates/demo");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(&sub).unwrap();

        fs::write(config_dir.join("AGENTS.md"), "global rule").unwrap();
        fs::write(repo.join("AGENTS.md"), "repo agents rule").unwrap();
        fs::write(repo.join("CLAUDE.md"), "repo claude rule").unwrap();
        // Identical duplicate in subdir must be deduplicated cleanly.
        fs::write(sub.join("AGENTS.md"), "repo agents rule").unwrap();
        // Oversized CLAUDE.local.md exercises UTF-8 safe truncation.
        let big = "规".repeat(7000); // 21,000 bytes (> 16 KiB)
        fs::write(sub.join("CLAUDE.local.md"), &big).unwrap();

        let mut stream = make_stream(true);
        apply_agents_md(&mut stream, &sub, &config_dir);

        let sys = match &stream.messages[0] {
            IrMessage::System { text, .. } => text.as_str(),
            _ => panic!("expected system message"),
        };
        assert!(sys.contains("<project_instructions>"));
        assert!(sys.contains("<agents_md path="));
        assert!(sys.contains("global rule"));
        assert!(sys.contains("repo agents rule"));
        assert_eq!(
            sys.matches("repo agents rule").count(),
            1,
            "duplicate content should be deduplicated"
        );
        assert!(sys.contains("<claude_md path="));
        assert!(sys.contains("repo claude rule"));
        assert!(sys.contains("truncated=\"true\""));
        assert!(sys.contains("</project_instructions>"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_memory_shares_key_across_git_worktrees_and_caps_index_at_200_lines() {
        let root = std::env::temp_dir().join(format!("anix-mem-test-{}", Uuid::new_v4().simple()));
        let main_repo = root.join("my-repo");
        let worktree = root.join("my-repo-wt");
        let data_dir = root.join("data");
        fs::create_dir_all(main_repo.join(".git/worktrees/wt1")).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(main_repo.join(".git/worktrees/wt1/commondir"), "../..\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!(
                "gitdir: {}\n",
                main_repo.join(".git/worktrees/wt1").display()
            ),
        )
        .unwrap();

        let key_main = project_memory_key(&main_repo);
        let key_wt = project_memory_key(&worktree);
        assert_eq!(key_main, key_wt, "worktree must share main repo memory key");
        assert!(key_main.starts_with("my-repo-"));

        let mem_dir = data_dir.join("memory").join(&key_main);
        fs::create_dir_all(&mem_dir).unwrap();
        let big_index: String = (1..=250)
            .map(|i| format!("- topic-{i}: summary line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(mem_dir.join("MEMORY.md"), &big_index).unwrap();

        let mut stream = make_stream(true);
        apply_memory(&mut stream, &worktree, &data_dir);
        let sys = match &stream.messages[0] {
            IrMessage::System { text, .. } => text.as_str(),
            _ => panic!("expected system message"),
        };
        assert!(sys.contains("<auto_memory dir="));
        assert!(sys.contains("<memory_index path="));
        assert!(sys.contains("truncated=\"true\""));
        assert!(sys.contains("topic-200"));
        assert!(!sys.contains("topic-201"));
        assert!(sys.contains("</auto_memory>"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tips_hook_emits_indexed_summaries_and_supports_override_and_mask() {
        let root = std::env::temp_dir().join(format!("anix-tips-test-{}", Uuid::new_v4().simple()));
        let cwd = root.join("repo");
        let cfg_dir = root.join("cfg");
        let state_dir = root.join("state");
        let rt_dir = root.join("rt");
        fs::create_dir_all(cwd.join(".anix/tips")).unwrap();
        fs::create_dir_all(&cfg_dir).unwrap();
        // Mask how-to-goal.md with a 0-byte file and add a custom project tip.
        fs::write(cwd.join(".anix/tips/how-to-goal.md"), b"").unwrap();
        fs::write(
            cwd.join(".anix/tips/how-to-deploy.md"),
            "# Deploy Checklist for Staging\nRun scripts/deploy.sh\n",
        )
        .unwrap();

        let mut stream = make_stream(true);
        apply_tips(&mut stream, &cwd, &cfg_dir, &state_dir, &rt_dir);
        let sys = match &stream.messages[0] {
            IrMessage::System { text, .. } => text.as_str(),
            _ => panic!("expected system message"),
        };
        assert!(sys.contains("<runtime_tips>"));
        assert!(sys.contains("how-to-sub-agent.md — "));
        assert!(sys.contains("$ANIX_WAL"));
        assert!(sys.contains("how-to-deploy.md — Deploy Checklist for Staging"));
        assert!(!sys.contains("how-to-goal.md"));

        let _ = fs::remove_dir_all(&root);
    }
}
