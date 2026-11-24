//! `/dev/tty` interactive `[y/n/a/e]` approval gate, `safe_commands` matching,
//! and Fail-Closed exit `42` when `/dev/tty` is unavailable (Section 5).

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::AsRawFd;
use std::process::{Command, Stdio};

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::ApprovalMode;

use crate::platform::posix;
use crate::signal::SignalState;

/// Decision returned by [`ApprovalGate::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Proceed with executing the (possibly edited) command string.
    Allow { cmd: String, source: &'static str },
    /// Denied without terminating `agent`; feed message back to model as `is_error: true`.
    DenyToModel {
        message: String,
        source: &'static str,
    },
    /// Hard Fail-Closed abort (`exit 42`).
    AbortExit42 { reason: String },
    /// Interrupted by `SIGINT` / `SIGTERM` while awaiting `/dev/tty` input.
    Interrupted(ExitCode),
}

/// Process-scoped approval gate with in-memory `Always` allowlist (never persisted to disk).
#[derive(Debug)]
pub struct ApprovalGate {
    mode: ApprovalMode,
    fail_on_deny: bool,
    safe_commands: Vec<String>,
    always_allowed: HashSet<String>,
}

impl ApprovalGate {
    pub fn new(mode: ApprovalMode, fail_on_deny: bool, safe_commands: Vec<String>) -> Self {
        Self {
            mode,
            fail_on_deny,
            safe_commands,
            always_allowed: HashSet::new(),
        }
    }

    /// Evaluate whether `cmd` is allowed to run under the active approval policy.
    pub fn check(&mut self, cmd: &str, signals: &SignalState) -> ApprovalDecision {
        if let Some(code) = signals.termination_exit_code() {
            return ApprovalDecision::Interrupted(code);
        }
        match self.mode {
            ApprovalMode::Auto => ApprovalDecision::Allow {
                cmd: cmd.to_owned(),
                source: "auto",
            },
            ApprovalMode::Deny => {
                if self.matches_safe_commands(cmd) {
                    ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "safe_commands",
                    }
                } else if self.fail_on_deny {
                    ApprovalDecision::AbortExit42 {
                        reason: format!("command denied by approval=deny (--fail-on-deny): {cmd}"),
                    }
                } else {
                    ApprovalDecision::DenyToModel {
                        message: "Error: Command denied by approval=deny policy".into(),
                        source: "deny_policy",
                    }
                }
            }
            ApprovalMode::Ask => {
                if self.matches_safe_commands(cmd) {
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "safe_commands",
                    };
                }
                if let Some(first_bin) = extract_argv0(cmd)
                    && self.always_allowed.contains(&first_bin)
                    && is_single_simple_command(cmd)
                {
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "always_allowlist",
                    };
                }
                self.prompt_dev_tty(cmd, signals)
            }
        }
    }

    /// Safe command allowlist check: every pipeline segment must match `safe_commands`
    /// and the command must NOT contain shell redirections, command substitutions, or
    /// unsafe chaining (`;`, `&&`, `||`, `&`, `\n`).
    fn matches_safe_commands(&self, cmd: &str) -> bool {
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
        segments.iter().all(|seg| self.segment_is_safe(seg))
    }

    fn segment_is_safe(&self, segment: &str) -> bool {
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

        self.safe_commands.iter().any(|allowed| {
            let allowed = allowed.trim();
            if allowed.contains(' ') {
                first_two.as_deref() == Some(allowed)
            } else {
                first == allowed
            }
        })
    }

    fn prompt_dev_tty(&mut self, cmd: &str, signals: &SignalState) -> ApprovalDecision {
        // Section 5: NEVER read/write `fd 0`; always open `/dev/tty` directly with `O_RDWR`.
        let Ok(tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
            return ApprovalDecision::AbortExit42 {
                reason: format!(
                    "approval=ask requires interactive /dev/tty (Fail-Closed); blocked command: {cmd}"
                ),
            };
        };
        let tty_fd = tty.as_raw_fd();
        let _ = posix::set_cloexec(tty_fd);

        let mut writer = match tty.try_clone() {
            Ok(w) => w,
            Err(_) => {
                return ApprovalDecision::AbortExit42 {
                    reason: format!("failed to clone /dev/tty; blocked command: {cmd}"),
                };
            }
        };
        let mut reader = BufReader::new(tty);

        loop {
            let _ = writeln!(writer, "\n[agent approval] Execute command:");
            let _ = writeln!(writer, "  $ {cmd}");
            let _ = write!(writer, "Allow? [y]es / [n]o / [a]lways / [e]dit: ");
            let _ = writer.flush();

            if let Some(code) = wait_tty_readable_or_signal(tty_fd, signals) {
                let _ = writeln!(writer);
                return ApprovalDecision::Interrupted(code);
            }

            let mut choice = String::new();
            match reader.read_line(&mut choice) {
                Ok(0) | Err(_) => {
                    if let Some(code) = signals.termination_exit_code() {
                        return ApprovalDecision::Interrupted(code);
                    }
                    return ApprovalDecision::AbortExit42 {
                        reason: format!("/dev/tty closed during approval; blocked command: {cmd}"),
                    };
                }
                Ok(_) => {}
            }

            let c = choice
                .trim()
                .chars()
                .next()
                .map(|ch| ch.to_ascii_lowercase());
            match c {
                Some('y') => {
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "tty_yes",
                    };
                }
                Some('n') => {
                    let _ = write!(writer, "Optional rejection reason (Enter to skip): ");
                    let _ = writer.flush();
                    if let Some(code) = wait_tty_readable_or_signal(tty_fd, signals) {
                        let _ = writeln!(writer);
                        return ApprovalDecision::Interrupted(code);
                    }
                    let mut reason = String::new();
                    let _ = reader.read_line(&mut reason);
                    let reason = reason.trim();
                    let msg = if reason.is_empty() {
                        "Error: User denied this action. Reason: (none provided)".into()
                    } else {
                        format!("Error: User denied this action. Reason: {reason}")
                    };
                    return ApprovalDecision::DenyToModel {
                        message: msg,
                        source: "tty_no",
                    };
                }
                Some('a') => {
                    if let Some(bin) = extract_argv0(cmd) {
                        self.always_allowed.insert(bin);
                    }
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "tty_always",
                    };
                }
                Some('e') => match edit_command_in_editor(cmd) {
                    Ok(Some(edited)) => {
                        return ApprovalDecision::Allow {
                            cmd: edited,
                            source: "tty_edit",
                        };
                    }
                    Ok(None) => {
                        return ApprovalDecision::DenyToModel {
                            message:
                                "Error: User denied this action. Reason: empty or aborted edit"
                                    .into(),
                            source: "tty_edit_aborted",
                        };
                    }
                    Err(err) => {
                        let _ = writeln!(writer, "Editor error: {err}");
                    }
                },
                _ => {
                    let _ = writeln!(writer, "Please enter y, n, a, or e.");
                }
            }
        }
    }
}

fn wait_tty_readable_or_signal(
    fd: std::os::unix::io::RawFd,
    signals: &SignalState,
) -> Option<ExitCode> {
    loop {
        if let Some(code) = signals.termination_exit_code() {
            return Some(code);
        }
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: Valid single-element pollfd slice.
        let rc = unsafe { libc::poll(&mut pfd, 1, 50) };
        if rc > 0 {
            return None;
        }
        if rc < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() != std::io::ErrorKind::Interrupted {
                return None;
            }
        }
    }
}

/// Write `cmd` to `/tmp/agent_edit_<uuid>.sh` (mode `0600`), invoke `$EDITOR` (fallback `vi`)
/// attached to `/dev/tty`, read modified command, and clean up the temporary file (Section 5.2).
fn edit_command_in_editor(cmd: &str) -> std::io::Result<Option<String>> {
    let (tmp_path, mut file) = posix::create_tmp_0600("agent_edit", "sh")?;
    file.write_all(cmd.as_bytes())?;
    if !cmd.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.flush()?;
    drop(file);

    let editor = std::env::var("EDITOR")
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

/// Split a shell command into pipe (`|`) segments ONLY if no unsafe shell metacharacters
/// (`;`, `&&`, `||`, `&`, `>`, `<`, `` ` ``, `$(`, `\n`) appear outside quotes.
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
            // Reject command substitution inside double quotes.
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
                // Reject `||` logical OR.
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

fn is_single_simple_command(cmd: &str) -> bool {
    split_safe_pipeline(cmd).is_some_and(|segs| segs.len() == 1)
}

fn extract_argv0(cmd: &str) -> Option<String> {
    let words = split_shell_words(cmd);
    for w in words {
        // Skip leading `KEY=VAL` env assignments.
        if w.contains('=') && !w.starts_with('=') && !w.starts_with('-') {
            continue;
        }
        return Some(w);
    }
    None
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
