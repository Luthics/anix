//! `anix-approve` — Layer 2 composable POSIX Shell approval wrapper (`docs/RUNTIME.md` §4).
//!
//! Usage:
//!   anix-approve [--mode ask|auto|deny] [--safe-cmd <PREFIX>]...
//!                [--decider <BIN>] [--decider-timeout <SEC>] [--fail-on-deny]
//!                -- <SHELL_BIN> [SHELL_ARGS...] -c <CMD>
//!
//! Exit semantics:
//! - Allowed: `exec()`s `<SHELL_BIN>` in-place (preserving FDs `$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL`/`$ANIX_INBOX`).
//! - Soft deny (model receives feedback and continues): writes reason to stdout and exits `126`.
//! - Hard Fail-Closed abort (`--fail-on-deny`, missing `/dev/tty` in `ask`, decider crash/timeout):
//!   writes diagnostic to stderr and exits `75` (`EX_TEMPFAIL`), signaling `anix` to abort with `75`.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode as StdExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anix_proto::exit_code::ExitCode;
use serde::Serialize;
use uuid::Uuid;

const DEFAULT_SAFE_COMMANDS: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "rg",
    "grep",
    "find",
    "pwd",
    "git status",
    "git diff",
    "git log",
];

const DENY_TO_MODEL_EXIT: u8 = 126;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ask,
    Auto,
    Deny,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Auto => "auto",
            Self::Deny => "deny",
        }
    }

    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ask" => Ok(Self::Ask),
            "auto" => Ok(Self::Auto),
            "deny" => Ok(Self::Deny),
            other => Err(format!("invalid mode `{other}` (expected ask|auto|deny)")),
        }
    }
}

#[derive(Debug)]
struct Args {
    mode: Mode,
    safe_commands: Vec<String>,
    decider: Option<String>,
    decider_timeout_sec: u64,
    fail_on_deny: bool,
    wrapped: Vec<String>,
}

#[derive(Serialize)]
struct DeciderInput<'a> {
    v: u32,
    cmd: &'a str,
    cwd: String,
    pid: u32,
    ppid: u32,
    approval_mode: &'a str,
}

fn parse_args() -> Result<Args, String> {
    let raw: Vec<String> = env::args().skip(1).collect();
    if raw.len() == 1 && (raw[0] == "--version" || raw[0] == "-V") {
        println!("anix-approve {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }
    if raw.len() == 1 && (raw[0] == "--help" || raw[0] == "-h") {
        println!(
            "Usage: anix-approve [--mode ask|auto|deny] [--safe-cmd <PREFIX>]... [--decider <BIN>] [--decider-timeout <SEC>] [--fail-on-deny] -- <SHELL_BIN> [SHELL_ARGS...] -c <CMD>"
        );
        std::process::exit(0);
    }

    let mut mode = env::var("ANIX_APPROVE_MODE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| Mode::parse(&s))
        .transpose()?
        .unwrap_or(Mode::Ask);
    let mut custom_safe: Vec<String> = Vec::new();
    let mut decider = env::var("ANIX_APPROVE_DECIDER")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let mut decider_timeout_sec: u64 = 30;
    let mut fail_on_deny = env::var("ANIX_APPROVE_FAIL_ON_DENY")
        .ok()
        .is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"));

    let mut i = 0;
    let mut sep_idx = None;
    while i < raw.len() {
        match raw[i].as_str() {
            "--" => {
                sep_idx = Some(i);
                break;
            }
            "--mode" => {
                i += 1;
                let val = raw.get(i).ok_or("--mode requires a value")?;
                mode = Mode::parse(val)?;
            }
            "--safe-cmd" => {
                i += 1;
                let val = raw.get(i).ok_or("--safe-cmd requires a value")?;
                custom_safe.push(val.clone());
            }
            "--decider" => {
                i += 1;
                let val = raw.get(i).ok_or("--decider requires a value")?;
                decider = Some(val.clone());
            }
            "--decider-timeout" => {
                i += 1;
                let val = raw.get(i).ok_or("--decider-timeout requires a value")?;
                decider_timeout_sec = val
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --decider-timeout `{val}`"))?
                    .max(1);
            }
            "--fail-on-deny" => {
                fail_on_deny = true;
            }
            other => {
                return Err(format!(
                    "unknown option `{other}` (use `--` before wrapped shell command)"
                ));
            }
        }
        i += 1;
    }

    let Some(sep) = sep_idx else {
        return Err("missing `-- <SHELL_BIN> [SHELL_ARGS...] -c <CMD>` separator".into());
    };
    let wrapped = raw[sep + 1..].to_vec();
    if wrapped.is_empty() {
        return Err("missing wrapped <SHELL_BIN> after `--`".into());
    }

    let safe_commands = if custom_safe.is_empty() {
        DEFAULT_SAFE_COMMANDS
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    } else {
        custom_safe
    };

    Ok(Args {
        mode,
        safe_commands,
        decider,
        decider_timeout_sec,
        fail_on_deny,
        wrapped,
    })
}

fn main() -> StdExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("anix-approve: {msg}");
            return ExitCode::UsageError.into();
        }
    };

    let (shell_spec, shell_args, cmd_idx) = extract_shell_and_cmd(&args.wrapped);
    let Some(cmd_idx) = cmd_idx else {
        eprintln!("anix-approve: wrapped command must contain `-c <CMD>`");
        return ExitCode::UsageError.into();
    };
    let cmd_str = shell_args[cmd_idx].clone();

    match evaluate(&args, &cmd_str) {
        Decision::Allow(final_cmd) => {
            let mut exec_args = shell_args;
            exec_args[cmd_idx] = final_cmd;
            let resolved_shell = resolve_bin(&shell_spec);
            let err = Command::new(&resolved_shell).args(&exec_args).exec();
            eprintln!(
                "anix-approve: failed to exec `{}`: {err}",
                resolved_shell.display()
            );
            ExitCode::RuntimeError.into()
        }
        Decision::DenyToModel(msg) => {
            println!("{msg}");
            StdExitCode::from(DENY_TO_MODEL_EXIT)
        }
        Decision::AbortTempFail(reason) => {
            eprintln!("anix-approve: {reason}");
            ExitCode::TempFail.into()
        }
    }
}

enum Decision {
    Allow(String),
    DenyToModel(String),
    AbortTempFail(String),
}

fn extract_shell_and_cmd(wrapped: &[String]) -> (String, Vec<String>, Option<usize>) {
    let shell_spec = wrapped[0].clone();
    let rest = wrapped[1..].to_vec();
    let mut cmd_idx = None;
    for idx in 0..rest.len() {
        if rest[idx] == "-c" && idx + 1 < rest.len() {
            cmd_idx = Some(idx + 1);
        }
    }
    (shell_spec, rest, cmd_idx)
}

fn evaluate(args: &Args, cmd: &str) -> Decision {
    match args.mode {
        Mode::Auto => Decision::Allow(cmd.to_owned()),
        Mode::Deny => {
            if matches_safe_commands(cmd, &args.safe_commands) {
                Decision::Allow(cmd.to_owned())
            } else if args.fail_on_deny {
                Decision::AbortTempFail(format!(
                    "command denied by approval=deny (--fail-on-deny): {cmd}"
                ))
            } else {
                Decision::DenyToModel("Error: Command denied by approval=deny policy".into())
            }
        }
        Mode::Ask => {
            if matches_safe_commands(cmd, &args.safe_commands) {
                return Decision::Allow(cmd.to_owned());
            }
            if let Some(ref decider_bin) = args.decider {
                if let Some(dec) =
                    run_external_decider(decider_bin, args.decider_timeout_sec, args.mode, cmd)
                {
                    return dec;
                }
            }
            prompt_dev_tty(cmd)
        }
    }
}

fn run_external_decider(
    bin_spec: &str,
    timeout_sec: u64,
    mode: Mode,
    cmd_str: &str,
) -> Option<Decision> {
    let bin_path = resolve_bin(bin_spec);
    let cwd = env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let ppid = unsafe { libc::getppid() as u32 };
    let payload = DeciderInput {
        v: 1,
        cmd: cmd_str,
        cwd,
        pid: std::process::id(),
        ppid,
        approval_mode: mode.as_str(),
    };
    let mut input_line = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(err) => {
            return Some(Decision::AbortTempFail(format!(
                "failed to serialize decider input: {err}"
            )));
        }
    };
    input_line.push('\n');

    let mut child = match Command::new(&bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(err) => {
            return Some(Decision::AbortTempFail(format!(
                "failed to execute decider `{}`: {err}",
                bin_path.display()
            )));
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let bytes = input_line.into_bytes();
        thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
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

    let deadline = Instant::now() + Duration::from_secs(timeout_sec.max(1));
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Some(Decision::AbortTempFail(format!(
                        "decider timed out after {timeout_sec}s (Fail-Closed); blocked command: {cmd_str}"
                    )));
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Some(Decision::AbortTempFail(format!(
                    "failed waiting on decider: {err}"
                )));
            }
        }
    };

    let stdout_str = out_handle.join().unwrap_or_default();
    let stderr_str = err_handle.join().unwrap_or_default();

    match status.code() {
        Some(0) => Some(Decision::Allow(cmd_str.to_owned())),
        Some(1) => {
            let reason = stdout_str.trim();
            let msg = if reason.is_empty() {
                "Error: Command denied by external approval policy".to_owned()
            } else {
                reason.to_owned()
            };
            Some(Decision::DenyToModel(msg))
        }
        Some(2) => None, // Escalate to `/dev/tty`
        other => {
            let detail = stderr_str.trim();
            let code_desc = other
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_owned());
            let reason = if detail.is_empty() {
                format!("decider exited with {code_desc} (Fail-Closed); blocked command: {cmd_str}")
            } else {
                format!("decider exited with {code_desc} (Fail-Closed): {detail}")
            };
            Some(Decision::AbortTempFail(reason))
        }
    }
}

fn prompt_dev_tty(cmd: &str) -> Decision {
    let Ok(tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        return Decision::AbortTempFail(format!(
            "approval=ask requires interactive /dev/tty (Fail-Closed); blocked command: {cmd}"
        ));
    };
    let mut writer = match tty.try_clone() {
        Ok(w) => w,
        Err(_) => {
            return Decision::AbortTempFail(format!(
                "failed to clone /dev/tty; blocked command: {cmd}"
            ));
        }
    };
    let mut reader = BufReader::new(tty);

    loop {
        let _ = writeln!(writer, "\n[anix-approve] Execute command:");
        let _ = writeln!(writer, "  $ {cmd}");
        let _ = write!(writer, "Allow? [y]es / [n]o / [e]dit: ");
        let _ = writer.flush();

        let mut choice = String::new();
        match reader.read_line(&mut choice) {
            Ok(0) | Err(_) => {
                return Decision::AbortTempFail(format!(
                    "/dev/tty closed during approval; blocked command: {cmd}"
                ));
            }
            Ok(_) => {}
        }

        let c = choice
            .trim()
            .chars()
            .next()
            .map(|ch| ch.to_ascii_lowercase());
        match c {
            Some('y') => return Decision::Allow(cmd.to_owned()),
            Some('n') => {
                let _ = write!(writer, "Optional rejection reason (Enter to skip): ");
                let _ = writer.flush();
                let mut reason = String::new();
                let _ = reader.read_line(&mut reason);
                let reason = reason.trim();
                let msg = if reason.is_empty() {
                    "Error: User denied this action. Reason: (none provided)".into()
                } else {
                    format!("Error: User denied this action. Reason: {reason}")
                };
                return Decision::DenyToModel(msg);
            }
            Some('e') => match edit_command_in_editor(cmd) {
                Ok(Some(edited)) => return Decision::Allow(edited),
                Ok(None) => {
                    return Decision::DenyToModel(
                        "Error: User denied this action. Reason: empty or aborted edit".into(),
                    );
                }
                Err(err) => {
                    let _ = writeln!(writer, "Editor error: {err}");
                }
            },
            _ => {
                let _ = writeln!(writer, "Please enter y, n, or e.");
            }
        }
    }
}

fn edit_command_in_editor(cmd: &str) -> std::io::Result<Option<String>> {
    let tmp_path = PathBuf::from(format!("/tmp/anix_edit_{}.sh", Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)?;
    file.write_all(cmd.as_bytes())?;
    if !cmd.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.flush()?;
    drop(file);

    let editor = env::var("EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "vi".into());

    let tty_in = OpenOptions::new().read(true).open("/dev/tty")?;
    let tty_out = OpenOptions::new().write(true).open("/dev/tty")?;
    let tty_err = OpenOptions::new().write(true).open("/dev/tty")?;

    let status = Command::new(&editor)
        .arg(&tmp_path)
        .stdin(Stdio::from(tty_in))
        .stdout(Stdio::from(tty_out))
        .stderr(Stdio::from(tty_err))
        .status();

    let edited = fs::read_to_string(&tmp_path).ok();
    let _ = fs::remove_file(&tmp_path);

    let status = status?;
    if !status.success() {
        return Ok(None);
    }
    let content = edited.unwrap_or_default();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_owned()))
    }
}

fn resolve_bin(spec: &str) -> PathBuf {
    if spec.contains('/') {
        if let Some(rest) = spec.strip_prefix("~/")
            && let Some(home) = env::var_os("HOME")
        {
            return PathBuf::from(home).join(rest);
        }
        return PathBuf::from(spec);
    }
    if let Some(path_var) = env::var_os("PATH") {
        for dir in env::split_paths(&path_var) {
            let cand = dir.join(spec);
            if is_executable(&cand) {
                return cand;
            }
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let libexec = PathBuf::from(home).join(".local/libexec/anix").join(spec);
        if is_executable(&libexec) {
            return libexec;
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join(spec);
        if is_executable(&sibling) {
            return sibling;
        }
    }
    PathBuf::from(spec)
}

fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

fn matches_safe_commands(cmd: &str, safe_commands: &[String]) -> bool {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return false;
    }
    let Some(segments) = split_safe_pipeline(trimmed) else {
        return false;
    };
    if segments.is_empty() {
        return false;
    }
    segments
        .iter()
        .all(|seg| segment_is_safe(seg, safe_commands))
}

fn segment_is_safe(segment: &str, safe_commands: &[String]) -> bool {
    let tokens = split_shell_words(segment);
    if tokens.is_empty() {
        return false;
    }
    let first = &tokens[0];
    let first_two = if tokens.len() >= 2 {
        Some(format!("{} {}", tokens[0], tokens[1]))
    } else {
        None
    };

    safe_commands.iter().any(|allowed| {
        let allowed = allowed.trim();
        if allowed.contains(' ') {
            first_two.as_deref() == Some(allowed)
        } else {
            first == allowed
        }
    })
}

fn split_safe_pipeline(cmd: &str) -> Option<Vec<String>> {
    let mut segments = Vec::new();
    let mut cur = String::new();
    let mut chars = cmd.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            cur.push(ch);
            continue;
        }
        if in_double {
            if ch == '\\' {
                cur.push(ch);
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
                continue;
            }
            if ch == '`' || (ch == '$' && chars.peek() == Some(&'(')) {
                return None;
            }
            if ch == '"' {
                in_double = false;
            }
            cur.push(ch);
            continue;
        }

        match ch {
            '\'' => {
                in_single = true;
                cur.push(ch);
            }
            '"' => {
                in_double = true;
                cur.push(ch);
            }
            '\\' => {
                cur.push(ch);
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            ';' | '&' | '>' | '<' | '`' | '\n' => return None,
            '$' if chars.peek() == Some(&'(') => return None,
            '|' => {
                if chars.peek() == Some(&'|') {
                    return None;
                }
                let seg = cur.trim().to_owned();
                if seg.is_empty() {
                    return None;
                }
                segments.push(seg);
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }

    if in_single || in_double {
        return None;
    }
    let last = cur.trim().to_owned();
    if !last.is_empty() {
        segments.push(last);
    }
    Some(segments)
}

fn split_shell_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                cur.push(ch);
            }
        } else if in_double {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            } else if ch == '"' {
                in_double = false;
            } else {
                cur.push(ch);
            }
        } else {
            match ch {
                '\'' => in_single = true,
                '"' => in_double = true,
                '\\' => {
                    if let Some(next) = chars.next() {
                        cur.push(next);
                    }
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        words.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(ch),
            }
        }
    }

    if !cur.is_empty() {
        words.push(cur);
    }
    words
}
