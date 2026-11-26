//! `anix-hook` — Layer 2 standalone `pre-context.d/` hook dispatcher (`20-skills` & `30-tips`).
//!
//! Invoked via BusyBox-style symlink name (`20-skills` / `30-tips`) or explicit flag
//! (`anix-hook --skills` / `anix-hook --tips`). Reads Canonical IR v1 JSONL from `stdin`
//! and writes updated Canonical IR v1 JSONL to `stdout`.

use std::collections::BTreeMap;
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

const BUILTIN_TIPS: &[(&str, &str)] = &[
    (
        "how-to-background.md",
        include_str!("../../../../runtime/tips/how-to-background.md"),
    ),
    (
        "how-to-goal.md",
        include_str!("../../../../runtime/tips/how-to-goal.md"),
    ),
    (
        "how-to-loop.md",
        include_str!("../../../../runtime/tips/how-to-loop.md"),
    ),
    (
        "how-to-mcp.md",
        include_str!("../../../../runtime/tips/how-to-mcp.md"),
    ),
    (
        "how-to-runtime.md",
        include_str!("../../../../runtime/tips/how-to-runtime.md"),
    ),
    (
        "how-to-sub-agent.md",
        include_str!("../../../../runtime/tips/how-to-sub-agent.md"),
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookMode {
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
    if base.contains("skills") {
        Some(HookMode::Skills)
    } else if base.contains("tips") {
        Some(HookMode::Tips)
    } else {
        None
    }
}

fn main() -> StdExitCode {
    let Some(mode) = detect_mode() else {
        eprintln!("Usage: anix-hook --skills | --tips (or symlink as 20-skills / 30-tips)");
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
        let runtime_dir = match env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
            Some(dir) => PathBuf::from(dir).join("anix"),
            None => {
                let uid = unsafe { libc::geteuid() };
                PathBuf::from(format!("/tmp/anix-{uid}"))
            }
        };

        match mode {
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

fn apply_skills(stream: &mut IrStream, cwd: &Path, home_dir: &Path, config_dir: &Path) {
    let search_dirs: Vec<PathBuf> = vec![
        cwd.join(".anix/skills"),
        cwd.join(".skills"),
        cwd.join(".claude/skills"),
        config_dir.join("skills"),
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
    let mut tips: BTreeMap<String, PathBuf> = BTreeMap::new();

    if let Some(builtin_dir) = materialize_builtin_tips(state_dir, runtime_dir) {
        for &(name, _) in BUILTIN_TIPS {
            let path = builtin_dir.join(name);
            if path.is_file() {
                let abs_path = fs::canonicalize(&path).unwrap_or(path);
                tips.insert(name.to_owned(), abs_path);
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
            tips.insert(name, abs_path);
        }
    }

    if tips.is_empty() {
        return;
    }

    let mut xml = String::from(
        "<runtime_tips>\nFor advanced POSIX workflows, read the relevant guide via shell when needed:\n",
    );
    for path in tips.values() {
        xml.push_str(&path.display().to_string());
        xml.push('\n');
    }
    xml.push_str("</runtime_tips>");
    append_to_first_system(stream, &xml);
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
        for &(name, content) in BUILTIN_TIPS {
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
