//! Approval gate: `safe_commands` allowlist, external `[approval].bin` decider (v1.1 §5.2),
//! `/dev/tty` interactive `[y/n/a/e]` prompt, and Fail-Closed `exit 75` (`EX_TEMPFAIL`) (Section 5).

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::hooks::{ApprovalDeciderInput, PLUGIN_WIRE_VERSION};
use os_agent_proto::ir::ApprovalMode;

use crate::config::{AgentPaths, ApprovalDeciderConfig};
use crate::platform::{self, ChildWaitOutcome, posix};
use crate::signal::SignalState;
use crate::trust;

/// Decision returned by [`ApprovalGate::check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// Proceed with executing the (possibly edited) command string.
    Allow {
        cmd: String,
        source: &'static str,
        decider: &'static str,
        decider_exit_code: Option<i32>,
    },
    /// Denied without terminating `agent`; feed message back to model as `is_error: true`.
    DenyToModel {
        message: String,
        source: &'static str,
        decider: &'static str,
        decider_exit_code: Option<i32>,
    },
    /// Hard Fail-Closed abort (`exit 75`, `EX_TEMPFAIL`).
    AbortTempFail {
        reason: String,
        decider: &'static str,
        decider_exit_code: Option<i32>,
    },
    /// Interrupted by `SIGINT` / `SIGTERM` while awaiting external decider or `/dev/tty`.
    Interrupted(ExitCode),
}

/// Process-scoped approval gate with in-memory `Always` allowlist (never persisted to disk)
/// and optional external `[approval].bin` decider (v1.1 §5.2).
#[derive(Debug)]
pub struct ApprovalGate {
    mode: ApprovalMode,
    fail_on_deny: bool,
    safe_commands: Vec<String>,
    decider: ApprovalDeciderConfig,
    agent_name: String,
    paths: AgentPaths,
    always_allowed: HashSet<String>,
}

impl ApprovalGate {
    pub fn new(
        mode: ApprovalMode,
        fail_on_deny: bool,
        safe_commands: Vec<String>,
        decider: ApprovalDeciderConfig,
        agent_name: impl Into<String>,
        paths: AgentPaths,
    ) -> Self {
        Self {
            mode,
            fail_on_deny,
            safe_commands,
            decider,
            agent_name: agent_name.into(),
            paths,
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
                decider: "builtin",
                decider_exit_code: None,
            },
            ApprovalMode::Deny => {
                if self.matches_safe_commands(cmd) {
                    ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "safe_commands",
                        decider: "builtin",
                        decider_exit_code: None,
                    }
                } else if self.fail_on_deny {
                    ApprovalDecision::AbortTempFail {
                        reason: format!("command denied by approval=deny (--fail-on-deny): {cmd}"),
                        decider: "builtin",
                        decider_exit_code: None,
                    }
                } else {
                    ApprovalDecision::DenyToModel {
                        message: "Error: Command denied by approval=deny policy".into(),
                        source: "deny_policy",
                        decider: "builtin",
                        decider_exit_code: None,
                    }
                }
            }
            ApprovalMode::Ask => {
                if self.matches_safe_commands(cmd) {
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "safe_commands",
                        decider: "builtin",
                        decider_exit_code: None,
                    };
                }
                if let Some(first_bin) = extract_argv0(cmd)
                    && self.always_allowed.contains(&first_bin)
                    && is_single_simple_command(cmd)
                {
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "always_allowlist",
                        decider: "builtin",
                        decider_exit_code: None,
                    };
                }
                if self.decider.bin.is_some() {
                    if let Some(decided) = self.run_external_decider(cmd, signals) {
                        return decided;
                    }
                    // Exit code 2: escalate back to kernel `/dev/tty` `[y/n/a/e]`.
                }
                self.prompt_dev_tty(cmd, signals)
            }
        }
    }

    /// Invoke external `[approval].bin` decider (v1.1 §5.2).
    /// Returns `None` ONLY when the decider exits with code `2` (escalate to `/dev/tty`).
    fn run_external_decider(
        &self,
        cmd_str: &str,
        signals: &SignalState,
    ) -> Option<ApprovalDecision> {
        let bin_spec = self.decider.bin.as_deref()?;
        let base_dir = self
            .paths
            .project_agent_dir
            .as_deref()
            .and_then(|d| d.parent());
        let bin_path = self.paths.resolve_component_bin(bin_spec, base_dir);

        if self.decider.bin_from_project
            && let Err(err) = trust::verify_trusted(&bin_path, &self.paths)
        {
            return Some(ApprovalDecision::AbortTempFail {
                reason: err.to_string(),
                decider: "external",
                decider_exit_code: None,
            });
        }

        let payload = ApprovalDeciderInput {
            v: PLUGIN_WIRE_VERSION,
            cmd: cmd_str.to_owned(),
            cwd: self.paths.cwd.display().to_string(),
            agent: self.agent_name.clone(),
            pid: std::process::id(),
            ppid: posix::current_ppid(),
            approval_mode: self.mode,
        };
        let mut input_line = match serde_json::to_string(&payload) {
            Ok(s) => s,
            Err(err) => {
                return Some(ApprovalDecision::AbortTempFail {
                    reason: format!("failed to serialize [approval].bin input: {err}"),
                    decider: "external",
                    decider_exit_code: None,
                });
            }
        };
        input_line.push('\n');

        let parent_pid = unsafe { libc::getpid() };
        let mut command = Command::new(&bin_path);
        command
            .current_dir(&self.paths.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // SAFETY: `pre_exec` invokes only async-signal-safe syscalls to isolate PGID and set PDEATHSIG.
        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                #[cfg(target_os = "linux")]
                {
                    crate::platform::linux::set_pdeathsig_sigkill(parent_pid)?;
                }
                #[cfg(not(target_os = "linux"))]
                let _ = parent_pid;
                Ok(())
            });
        }

        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(err) => {
                return Some(ApprovalDecision::AbortTempFail {
                    reason: format!(
                        "failed to execute [approval].bin `{}`: {err}",
                        bin_path.display()
                    ),
                    decider: "external",
                    decider_exit_code: None,
                });
            }
        };

        let child_pgid = child.id() as libc::pid_t;
        signals.set_active_child_pgid(child_pgid);

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

        let timeout = Duration::from_secs(self.decider.timeout_sec.max(1));
        let deadline = Instant::now() + timeout;

        let status = loop {
            if let Some(code) = signals.termination_exit_code() {
                let _ = signals.terminate_active_child_and_wait(&mut child);
                signals.clear_active_child_pgid();
                return Some(ApprovalDecision::Interrupted(code));
            }
            match platform::wait_child_until(&mut child, Some(deadline), Some(signals.wakeup_fd()))
            {
                Ok(ChildWaitOutcome::Exited(st)) => break st,
                Ok(ChildWaitOutcome::SignalWakeup) => {
                    signals.drain_wakeup();
                    continue;
                }
                Ok(ChildWaitOutcome::TimedOut) => {
                    let _ = signals.terminate_active_child_and_wait(&mut child);
                    signals.clear_active_child_pgid();
                    let stderr_str = err_handle.join().unwrap_or_default();
                    let detail = stderr_str.trim();
                    let reason = if detail.is_empty() {
                        format!(
                            "[approval].bin timed out after {}s (Fail-Closed); blocked command: {cmd_str}",
                            self.decider.timeout_sec
                        )
                    } else {
                        format!(
                            "[approval].bin timed out after {}s (Fail-Closed): {detail}",
                            self.decider.timeout_sec
                        )
                    };
                    return Some(ApprovalDecision::AbortTempFail {
                        reason,
                        decider: "external",
                        decider_exit_code: None,
                    });
                }
                Err(err) => {
                    let _ = signals.terminate_active_child_and_wait(&mut child);
                    signals.clear_active_child_pgid();
                    return Some(ApprovalDecision::AbortTempFail {
                        reason: format!("failed waiting on [approval].bin: {err}"),
                        decider: "external",
                        decider_exit_code: None,
                    });
                }
            }
        };

        signals.clear_active_child_pgid();
        let stdout_str = out_handle.join().unwrap_or_default();
        let stderr_str = err_handle.join().unwrap_or_default();

        match status.code() {
            Some(0) => Some(ApprovalDecision::Allow {
                cmd: cmd_str.to_owned(),
                source: "external_allow",
                decider: "external",
                decider_exit_code: Some(0),
            }),
            Some(1) => {
                let reason = stdout_str.trim();
                let message = if reason.is_empty() {
                    "Error: Command denied by external approval policy".to_owned()
                } else {
                    reason.to_owned()
                };
                Some(ApprovalDecision::DenyToModel {
                    message,
                    source: "external_deny",
                    decider: "external",
                    decider_exit_code: Some(1),
                })
            }
            Some(2) => None, // Escalate to kernel `/dev/tty` `[y/n/a/e]`
            other => {
                let detail = stderr_str.trim();
                let code_desc = other
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".to_owned());
                let reason = if detail.is_empty() {
                    format!(
                        "[approval].bin exited with {code_desc} (Fail-Closed); blocked command: {cmd_str}"
                    )
                } else {
                    format!("[approval].bin exited with {code_desc} (Fail-Closed): {detail}")
                };
                Some(ApprovalDecision::AbortTempFail {
                    reason,
                    decider: "external",
                    decider_exit_code: other,
                })
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
            return ApprovalDecision::AbortTempFail {
                reason: format!(
                    "approval=ask requires interactive /dev/tty (Fail-Closed); blocked command: {cmd}"
                ),
                decider: "builtin",
                decider_exit_code: None,
            };
        };
        let tty_fd = tty.as_raw_fd();
        let _ = posix::set_cloexec(tty_fd);

        let mut writer = match tty.try_clone() {
            Ok(w) => w,
            Err(_) => {
                return ApprovalDecision::AbortTempFail {
                    reason: format!("failed to clone /dev/tty; blocked command: {cmd}"),
                    decider: "builtin",
                    decider_exit_code: None,
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
                    return ApprovalDecision::AbortTempFail {
                        reason: format!("/dev/tty closed during approval; blocked command: {cmd}"),
                        decider: "builtin",
                        decider_exit_code: None,
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
                        decider: "builtin",
                        decider_exit_code: None,
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
                        decider: "builtin",
                        decider_exit_code: None,
                    };
                }
                Some('a') => {
                    if let Some(bin) = extract_argv0(cmd) {
                        self.always_allowed.insert(bin);
                    }
                    return ApprovalDecision::Allow {
                        cmd: cmd.to_owned(),
                        source: "tty_always",
                        decider: "builtin",
                        decider_exit_code: None,
                    };
                }
                Some('e') => match edit_command_in_editor(cmd) {
                    Ok(Some(edited)) => {
                        return ApprovalDecision::Allow {
                            cmd: edited,
                            source: "tty_edit",
                            decider: "builtin",
                            decider_exit_code: None,
                        };
                    }
                    Ok(None) => {
                        return ApprovalDecision::DenyToModel {
                            message:
                                "Error: User denied this action. Reason: empty or aborted edit"
                                    .into(),
                            source: "tty_edit_aborted",
                            decider: "builtin",
                            decider_exit_code: None,
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
        match posix::poll_readable_or_signal(fd, signals.wakeup_fd(), None) {
            Ok(posix::PollFdOutcome::FdReady) => return None,
            Ok(posix::PollFdOutcome::SignalWakeup) => {
                signals.drain_wakeup();
                if let Some(code) = signals.termination_exit_code() {
                    return Some(code);
                }
            }
            Ok(posix::PollFdOutcome::TimedOut) | Err(_) => return None,
        }
    }
}

/// Write `cmd` to `/tmp/agent_edit_<uuid>.sh` (mode `0600`), invoke `$EDITOR` (fallback `vi`)
/// attached to `/dev/tty`, read modified command, and clean up the temporary file (Section 5.3).
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

fn is_single_simple_command(cmd: &str) -> bool {
    split_safe_pipeline(cmd).is_some_and(|segs| segs.len() == 1)
}

fn extract_argv0(cmd: &str) -> Option<String> {
    let words = split_shell_words(cmd);
    for w in words {
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
