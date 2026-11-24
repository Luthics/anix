//! §§4.1, 5.1–5.3. The terminal is local to the child session; the test runner's
//! own stdin, controlling terminal and interactive user are never used.
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::{fs::PermissionsExt, process::CommandExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::{Fixture, expect_code, jsonl, scripted_provider, success_text, tool, with_tools};

struct Pty {
    master: File,
    slave: File,
}
impl Pty {
    fn open() -> Self {
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: The output descriptors are distinct, live pointers; null optional
        // arguments request the platform defaults for terminal attributes and size.
        let rc = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(rc, 0, "openpty: {}", io::Error::last_os_error());
        // SAFETY: openpty succeeded and transferred ownership of both descriptors.
        let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        for f in [&master, &slave] {
            // SAFETY: f owns a live descriptor. F_SETFD accepts FD_CLOEXEC.
            assert_eq!(
                unsafe { libc::fcntl(f.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
                0
            );
        }
        // SAFETY: F_GETFL and F_SETFL operate on the live master descriptor.
        let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
        assert!(flags >= 0);
        // SAFETY: Preserve the file status flags while enabling non-blocking reads.
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        Self { master, slave }
    }
    fn spawn(self, mut cmd: Command, root: &Path) -> Running {
        let fd = self.slave.as_raw_fd();
        // SAFETY: pre_exec only calls setsid and ioctl on the live slave descriptor.
        // Both descriptors have CLOEXEC and remain owned by self until spawn returns.
        unsafe {
            cmd.pre_exec(move || {
                if libc::setsid() == -1 || libc::ioctl(fd, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().unwrap();
        Running {
            child,
            master: Some(self.master),
            slave: Some(self.slave),
            result: root.join("terminal-status"),
        }
    }
}

fn terminal_command(fx: &Fixture) -> Command {
    // As in a user's shell, the session leader outlives the foreground command.
    // Publish its status before the kernel tears down the controlling terminal.
    let mut cmd = fx.command_for("/bin/sh");
    cmd.args([
        "-c",
        r#"
# The PTY cases supply exactly one input on fd 0. Do not turn a runner's
# inherited descriptors into additional RFC §4.1 user messages.
"$@" 3>&- 4<&- 5<&- 6<&- 7<&- 8<&- 9<&-
rc=$?
trap '' HUP
printf '%s' "$rc" > "$HOME/status.tmp"
/bin/mv "$HOME/status.tmp" "$HOME/terminal-status"
exit "$rc"
"#,
        "terminal-session",
        env!("CARGO_BIN_EXE_agent"),
    ]);
    cmd
}

struct Running {
    child: Child,
    master: Option<File>,
    // Keep the terminal connected when all three standard streams are redirected.
    slave: Option<File>,
    result: PathBuf,
}
impl Running {
    fn send(&mut self, bytes: &[u8]) {
        self.master.as_mut().unwrap().write_all(bytes).unwrap();
    }
    fn until(&mut self, needle: &str) {
        let start = Instant::now();
        let mut seen = Vec::new();
        let mut chunk = [0; 2048];
        loop {
            match self.master.as_mut().unwrap().read(&mut chunk) {
                Ok(n) if n > 0 => {
                    seen.extend_from_slice(&chunk[..n]);
                    if String::from_utf8_lossy(&seen).contains(needle) {
                        return;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                other => panic!(
                    "terminal read {other:?}: {}",
                    String::from_utf8_lossy(&seen)
                ),
            }
            assert!(
                start.elapsed() < Duration::from_secs(12),
                "waiting for {needle:?}: {}",
                String::from_utf8_lossy(&seen)
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn finish(&mut self, code: i32) {
        assert_eq!(self.wait_for_exit(), code);
    }
    fn wait_for_exit(&mut self) -> i32 {
        let end = Instant::now() + Duration::from_secs(12);
        loop {
            if self.result.exists() {
                self.slave.take();
                self.master.take();
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                let code = status.code().unwrap();
                assert_eq!(fs::read_to_string(&self.result).unwrap(), code.to_string());
                return code;
            }
            assert!(
                Instant::now() < end,
                "agent did not finish after terminal interaction"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.slave.take();
        self.master.take();
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

// RFC §4.1
#[test]
fn tty_stdin_uses_args_immediately_or_reads_input_through_eof() {
    for args_present in [true, false] {
        let fx = Fixture::new();
        let pty = Pty::open();
        fs::write(fx.dir.0.join("case.agent"), "test").unwrap();
        let mut cmd = terminal_command(&fx);
        cmd.arg("case.agent")
            .stdin(Stdio::from(pty.slave.try_clone().unwrap()))
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if args_present {
            cmd.args(["--", "arg-only"]);
        }
        let mut running = pty.spawn(cmd, &fx.dir.0);
        if !args_present {
            running.send(b"terminal message\n\x04");
        }
        running.finish(0);
        let ir = jsonl(&fx.capture);
        assert_eq!(
            ir.last().unwrap()["text"],
            if args_present {
                "<args>arg-only</args>"
            } else {
                "terminal message\n"
            }
        );
    }
}

// RFC §4.1, §5.1, §5.2
#[test]
fn approval_choices_use_dev_tty_and_preserve_piped_stdin() {
    // Exercise optional rejection text and both documented editor sources.
    for (choice, reason, fallback_vi) in [
        ("y", "", false),
        ("n", "fixture refusal", false),
        ("n", "", false),
        ("e", "", false),
        ("e", "", true),
        ("a", "", false),
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "test").unwrap();
        let edit = root.join(if fallback_vi { "vi" } else { "editor" });
        fs::write(
            &edit,
            r#"#!/bin/sh
set -eu
printf '%s' "$1" > "$HOME/edit-path"
/bin/cat "$1" > "$HOME/edit-before"
/bin/ls -ld "$1" > "$HOME/edit-mode"
printf 'printf edited' > "$1"
"#,
        )
        .unwrap();
        fs::set_permissions(&edit, fs::Permissions::from_mode(0o700)).unwrap();
        let mut turns = vec![with_tools([tool("first", "printf original")])];
        if choice == "a" {
            turns.push(with_tools([tool("second", "printf second")]));
        }
        turns.push(success_text("done"));
        scripted_provider(&fx, &turns);
        let pty = Pty::open();
        let mut c = terminal_command(&fx);
        if !fallback_vi {
            c.env("EDITOR", &edit);
        }
        c.args(["--approval", "ask", "--shell", "/bin/sh", "case.agent"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut running = pty.spawn(c, root);
        running
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"from pipeline")
            .unwrap();
        running.until("printf original");
        running.send(format!("{choice}\n").as_bytes());
        if choice == "n" {
            // Wait until the process actually requests the reason before sending another line.
            running.until("reason");
            running.send(format!("{reason}\n").as_bytes());
        }
        running.finish(0);
        let ir = jsonl(&root.join("request-2"));
        let inputs: Vec<_> = ir
            .iter()
            .filter(|m| m["role"] == "user")
            .map(|m| &m["text"])
            .collect();
        assert_eq!(inputs, ["from pipeline"], "approval choice {choice}");
        let result = ir.last().unwrap();
        assert_eq!(result["role"], "tool");
        let text = result["text"].as_str().unwrap();
        match choice {
            "n" => {
                assert_eq!(result["is_error"], true);
                assert!(text.contains("Error: User denied this action."), "{text}");
                if !reason.is_empty() {
                    assert!(text.contains(&format!("Reason: {reason}")), "{text}");
                }
            }
            "e" => {
                assert!(text.ends_with("edited"));
                assert_eq!(
                    fs::read_to_string(root.join("edit-before")).unwrap().trim(),
                    "printf original"
                );
                let path = fs::read_to_string(root.join("edit-path")).unwrap();
                assert!(path.starts_with("/tmp/agent_edit_") && path.ends_with(".sh"));
                assert!(
                    fs::read_to_string(root.join("edit-mode"))
                        .unwrap()
                        .starts_with("-rw-------")
                );
                assert!(!std::path::Path::new(&path).exists());
            }
            "a" => {
                let next = jsonl(&root.join("request-3"));
                assert!(
                    next.last().unwrap()["text"]
                        .as_str()
                        .unwrap()
                        .ends_with("second")
                );
            }
            _ => assert!(text.ends_with("original")),
        }
    }
}

// RFC §5.2
#[test]
fn unsuccessful_or_empty_terminal_edit_runs_neither_original_nor_edited_command() {
    for edit in [
        r#"printf 'printf edited > ran' > "$1"; exit 1"#,
        r#": > "$1""#,
    ] {
        let fx = Fixture::new();
        let root = &fx.dir.0;
        fs::write(root.join("case.agent"), "test").unwrap();
        let editor = root.join("editor");
        super::executable(
            &editor,
            &format!("#!/bin/sh\nprintf '%s' \"$1\" > edit-path\n{edit}\n"),
        );
        scripted_provider(
            &fx,
            &[
                with_tools([tool("edit", "printf original > ran")]),
                success_text("finished"),
            ],
        );
        let mut cmd = terminal_command(&fx);
        cmd.env("EDITOR", editor)
            .args(["--shell", "/bin/sh", "case.agent", "--", "go"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = Pty::open().spawn(cmd, root);
        child.until("printf original");
        // An implementation can decline the edit or prompt again. A queued
        // refusal also terminates that second interaction without approving it.
        child.send(b"e\nn\n\n");
        child.wait_for_exit();
        let path = fs::read_to_string(root.join("edit-path")).unwrap();
        assert!(!Path::new(&path).exists());
        assert!(!root.join("ran").exists());
    }
}

// RFC §5.1, §5.2
#[test]
fn always_approval_is_scoped_to_executable_and_pid_even_with_the_same_wal() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "test").unwrap();
    let first = with_tools([
        tool("one", "printf first"),
        tool("two", "printf second"),
        tool("other", "echo other"),
    ]);
    scripted_provider(
        &fx,
        &[
            first,
            success_text("done"),
            with_tools([tool("new", "printf next-pid")]),
            success_text("done"),
        ],
    );
    for (n, prompt) in ["printf first", "printf next-pid"].iter().enumerate() {
        let mut cmd = terminal_command(&fx);
        cmd.args([
            "--approval",
            "ask",
            "--shell",
            "/bin/sh",
            "-c",
            "history",
            "case.agent",
            "--",
            "go",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        let mut child = Pty::open().spawn(cmd, root);
        child.until(prompt);
        child.send(if n == 0 { b"a\n" } else { b"y\n" });
        if n == 0 {
            child.until("echo other");
            child.send(b"y\n");
        }
        child.finish(0);
        fs::remove_file(root.join("terminal-status")).unwrap();
    }
    let rows = jsonl(&root.join("history"));
    let results: Vec<_> = rows
        .iter()
        .filter(|r| r["role"] == "tool")
        .map(|r| {
            r["text"]
                .as_str()
                .unwrap()
                .split_once('\n')
                .unwrap()
                .1
                .trim_end()
        })
        .collect();
    assert_eq!(results, ["first", "second", "other", "next-pid"]);
}

// RFC §5.3, §6.4
#[test]
fn detached_session_requiring_approval_fails_with_42_without_executing_tool() {
    let fx = Fixture::new();
    let root = &fx.dir.0;
    fs::write(root.join("case.agent"), "test").unwrap();
    scripted_provider(&fx, &[with_tools([tool("t", "printf unapproved > ran")])]);
    let mut cmd = fx.agent();
    cmd.args([
        "--approval",
        "ask",
        "--shell",
        "/bin/sh",
        "case.agent",
        "--",
        "go",
    ]);
    // SAFETY: setsid is the only syscall in this child hook. The parent keeps
    // ordinary stdin connected to /dev/null, and the child has no controlling tty.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let out = fx.run(&mut cmd);
    expect_code(&out, 42);
    assert!(!root.join("ran").exists());
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("printf unapproved > ran")
    );
}
