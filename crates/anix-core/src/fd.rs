//! External FD 0/1/2/4..9 contract, `fd 2` newline tracking, `FD_CLOEXEC` hygiene,
//! anonymous pipe relays (`$ANIX_OUT`, `$ANIX_ERR`, `$ANIX_CTL`), and pure event-driven
//! shell executor spawning (RFC §§3.4, 4.1, 4.2 & 6.1).

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use anix_proto::ctl::{CtlFrame, MAX_CTL_FRAME_BYTES};
use anix_proto::exit_code::ExitCode;

use crate::platform::{self, ChildWaitOutcome, PreparedSandbox, posix};
use crate::signal::SignalState;

const PIPE_DRAIN_AFTER_EXIT: Duration = Duration::from_millis(200);

/// Tracks the trailing byte written to `fd 2` (`stderr`) to enforce the
/// deterministic newline contract without ever touching `fd 1` (`stdout`).
#[derive(Debug, Clone, Default)]
pub struct StderrTracker {
    needs_newline: Arc<AtomicBool>,
}

impl StderrTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn note_bytes(&self, bytes: &[u8]) {
        if let Some(&last) = bytes.last() {
            self.needs_newline.store(last != b'\n', Ordering::SeqCst);
        }
    }

    pub fn ensure_newline(&self) {
        if self.needs_newline.swap(false, Ordering::SeqCst) {
            let mut err = io::stderr().lock();
            let _ = err.write_all(b"\n");
            let _ = err.flush();
        }
    }

    pub fn writeln_line(&self, line: &str) {
        self.ensure_newline();
        let mut err = io::stderr().lock();
        let _ = writeln!(err, "{line}");
        let _ = err.flush();
        self.needs_newline.store(false, Ordering::SeqCst);
    }

    pub fn writeln_dim_if_tty(&self, line: &str) {
        self.ensure_newline();
        let mut err = io::stderr().lock();
        if posix::isatty(2) {
            let _ = writeln!(err, "\x1b[2m{line}\x1b[0m");
        } else {
            let _ = writeln!(err, "{line}");
        }
        let _ = err.flush();
        self.needs_newline.store(false, Ordering::SeqCst);
    }
}

/// Drain and immediately close any inherited readable static multi-input FDs (`fd 4..=9`),
/// skipping `skip_fd` if `--trace-fd=<N>` claimed a descriptor in that range.
pub fn drain_inherited_extra_fds(skip_fd: Option<RawFd>) -> Vec<(RawFd, String)> {
    let mut extras = Vec::new();
    for fd in 4..=9 {
        if skip_fd == Some(fd) {
            continue;
        }
        if posix::fd_is_readable(fd) {
            let mut buf = Vec::new();
            // SAFETY: `fd` is verified open & readable; wrapping in `File` owns and closes it on drop.
            let mut file = unsafe { File::from_raw_fd(fd) };
            let _ = file.read_to_end(&mut buf);
            drop(file);
            extras.push((fd, String::from_utf8_lossy(&buf).into_owned()));
        }
    }
    extras
}

/// Assemble initial user input from `<SCRIPT_ARGS>`, `fd 0` (`stdin`), and pre-drained `fd 4..=9`.
pub fn assemble_initial_inputs(
    script_args: &[String],
    extra_fds: &[(RawFd, String)],
    skip_tty_stdin: bool,
    signals: &SignalState,
) -> io::Result<String> {
    let mut sections: Vec<String> = Vec::new();

    if !script_args.is_empty() {
        sections.push(script_args.join(" "));
    }

    let is_tty_stdin = posix::isatty(0);
    let should_read_stdin = !(is_tty_stdin && (!script_args.is_empty() || skip_tty_stdin));
    if should_read_stdin {
        let stdin_bytes = read_fd_interruptible(0, signals)?;
        let stdin_text = String::from_utf8_lossy(&stdin_bytes).into_owned();
        if !stdin_text.is_empty() {
            sections.push(stdin_text);
        }
    }

    for (fd, text) in extra_fds {
        sections.push(format!("<input fd=\"{fd}\">\n{text}\n</input>"));
    }

    Ok(sections.join("\n\n"))
}

pub(crate) fn read_fd_interruptible(fd: RawFd, signals: &SignalState) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if signals.termination_exit_code().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "interrupted by signal while reading stdin",
            ));
        }
        match posix::poll_readable_or_signal(fd, signals.wakeup_fd(), None)? {
            posix::PollFdOutcome::SignalWakeup => {
                signals.drain_wakeup();
                continue;
            }
            posix::PollFdOutcome::TimedOut => continue,
            posix::PollFdOutcome::FdReady => {}
        }
        // SAFETY: `buf` is a valid writable buffer for `read(2)`.
        let n = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n as usize]);
    }
    Ok(out)
}

/// Output captured from a single shell invocation (`<program> <prefix_args...> -c "<cmd>"`).
#[derive(Debug)]
pub struct ShellOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub anix_out_bytes: Vec<u8>,
    pub ctl_frames: Vec<CtlFrame>,
    pub ctl_warnings: Vec<String>,
    pub interrupted_by: Option<ExitCode>,
}

/// Read-only runtime context channels actively injected into shell children after clearing
/// inherited `ANIX_*` environment variables (RFC §3.4: child `anix` processes NEVER read these
/// implicitly as their own CLI/config settings).
#[derive(Debug, Clone, Copy, Default)]
pub struct ShellRuntimeEnv<'a> {
    pub parent_inbox: Option<&'a Path>,
    pub wal_path: Option<&'a Path>,
    pub fork_leaf: Option<u64>,
    pub agent_script: Option<&'a Path>,
}

/// Spawn `<shell_bin> <shell_prefix_args...> -c <cmd>` with anonymous pipe relays for
/// `$ANIX_OUT` (when `mount_anix_out`), `$ANIX_ERR`, `$ANIX_CTL`, active `$ANIX_INBOX` /
/// `$ANIX_WAL` / `$ANIX_LEAF` / `$ANIX_AGENT` injection, and pure event-driven wait (`pidfd`/`kqueue`).
#[allow(clippy::too_many_arguments)]
pub fn run_shell_command(
    shell_bin: &Path,
    shell_prefix_args: &[String],
    tools_dir: &Path,
    cmd_str: &str,
    tool_args_json: &str,
    cwd: &Path,
    exec_timeout_sec: Option<u64>,
    exec_kill_on_timeout: Option<bool>,
    runtime_env: ShellRuntimeEnv<'_>,
    mount_anix_out: bool,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
    stderr_tracker: &StderrTracker,
) -> io::Result<ShellOutput> {
    let out_pipe = if mount_anix_out {
        Some(posix::create_pipe_cloexec()?)
    } else {
        None
    };
    let (err_r, err_w) = posix::create_pipe_cloexec()?;
    let (ctl_r, ctl_w) = posix::create_pipe_cloexec()?;

    #[cfg(target_os = "linux")]
    unsafe {
        if let Some((_, out_w)) = out_pipe {
            let _ = libc::fchmod(out_w, 0o200);
        }
        let _ = libc::fchmod(err_w, 0o200);
    }

    // SAFETY: Pipe read ends are exclusively owned by their respective relay threads.
    let mut err_reader = unsafe { File::from_raw_fd(err_r) };
    let mut ctl_reader = unsafe { File::from_raw_fd(ctl_r) };

    let relay_active = Arc::new(AtomicBool::new(true));

    let (out_w_opt, out_done_rx) = if let Some((out_r, out_w)) = out_pipe {
        // SAFETY: `out_r` is exclusively owned by this relay thread.
        let mut out_reader = unsafe { File::from_raw_fd(out_r) };
        let (out_done_tx, rx) = mpsc::channel::<Vec<u8>>();
        let out_active = Arc::clone(&relay_active);
        thread::spawn(move || {
            const MAX_CAPTURED_ANIX_OUT: usize = 1024 * 1024;
            let mut buf = [0u8; 8192];
            let mut captured = Vec::new();
            loop {
                match out_reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if out_active.load(Ordering::Acquire) {
                            let mut stdout = io::stdout().lock();
                            let _ = stdout.write_all(&buf[..n]);
                            let _ = stdout.flush();
                            if captured.len() < MAX_CAPTURED_ANIX_OUT {
                                let take = n.min(MAX_CAPTURED_ANIX_OUT - captured.len());
                                captured.extend_from_slice(&buf[..take]);
                            }
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            let _ = out_done_tx.send(captured);
        });
        (Some(out_w), Some(rx))
    } else {
        (None, None)
    };

    let (err_done_tx, err_done_rx) = mpsc::channel::<()>();
    let err_tracker = stderr_tracker.clone();
    let err_active = Arc::clone(&relay_active);
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut wrote_any = false;
        loop {
            match err_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if err_active.load(Ordering::Acquire) {
                        if !wrote_any {
                            err_tracker.ensure_newline();
                            wrote_any = true;
                        }
                        let mut stderr = io::stderr().lock();
                        let _ = stderr.write_all(&buf[..n]);
                        let _ = stderr.flush();
                        err_tracker.note_bytes(&buf[..n]);
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let _ = err_done_tx.send(());
    });

    let (ctl_tx, ctl_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        pump_reader_to_channel(&mut ctl_reader, &ctl_tx);
    });

    // Prepend private tools dir (`~/.local/libexec/anix/tools`) to child PATH (RFC §2).
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path_entries = vec![tools_dir.to_path_buf()];
    path_entries.extend(std::env::split_paths(&existing_path));
    let new_path = std::env::join_paths(path_entries).unwrap_or(existing_path);

    let parent_pid = unsafe { libc::getpid() };
    let sandbox_cloned = sandbox.cloned();

    let mut command = Command::new(shell_bin);
    command
        .args(shell_prefix_args)
        .arg("-c")
        .arg(cmd_str)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // RFC §3.4: Clear ALL `ANIX_*` environment variables except `ANIX_LOG` & `ANIX_TRACE_LEVEL`,
    // then actively inject only the authoritative runtime channels.
    for (key, _) in std::env::vars_os() {
        let key_str = key.to_string_lossy();
        if key_str.starts_with("ANIX_") && key_str != "ANIX_LOG" && key_str != "ANIX_TRACE_LEVEL" {
            command.env_remove(&key);
        }
    }

    command
        .env("PATH", new_path)
        .env("ANIX_ERR", "/dev/fd/5")
        .env("ANIX_CTL", "/dev/fd/6")
        .env("ANIX_TOOL_ARGS", tool_args_json);
    if mount_anix_out {
        command.env("ANIX_OUT", "/dev/fd/4");
    } else {
        command.env_remove("ANIX_OUT");
    }
    if let Some(inbox_dir) = runtime_env.parent_inbox {
        command.env("ANIX_INBOX", inbox_dir);
    }
    if let Some(wal_path) = runtime_env.wal_path {
        command.env("ANIX_WAL", wal_path);
    }
    if let Some(leaf_id) = runtime_env.fork_leaf {
        command.env("ANIX_LEAF", leaf_id.to_string());
    }
    if let Some(agent_script) = runtime_env.agent_script {
        command.env("ANIX_AGENT", agent_script);
    }
    if let Some(t) = exec_timeout_sec {
        command.env("ANIX_EXEC_TIMEOUT_SEC", t.to_string());
    }
    if let Some(k) = exec_kill_on_timeout {
        command.env("ANIX_EXEC_KILL_ON_TIMEOUT", if k { "1" } else { "0" });
    }

    // SAFETY: `pre_exec` invokes only async-signal-safe syscalls to isolate PGID, bind FDs,
    // set Linux `PR_SET_PDEATHSIG`, and apply Landlock rules.
    unsafe {
        command.pre_exec(move || {
            if libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }

            #[cfg(target_os = "linux")]
            {
                crate::platform::linux::set_pdeathsig_sigkill(parent_pid)?;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = parent_pid;

            let tmp_out = match out_w_opt {
                Some(ow) => Some(dup_above_6(ow)?),
                None => None,
            };
            let tmp_err = dup_above_6(err_w)?;
            let tmp_ctl = dup_above_6(ctl_w)?;

            if let Some(to) = tmp_out {
                dup2_inheritable(to, 4)?;
            } else {
                let _ = libc::close(4);
            }
            dup2_inheritable(tmp_err, 5)?;
            dup2_inheritable(tmp_ctl, 6)?;

            if let Some(ref sb) = sandbox_cloned {
                platform::apply_prepared_sandbox_in_child(sb)?;
            }

            Ok(())
        });
    }

    let spawn_res = command.spawn();

    // Parent MUST close its copies of pipe write ends immediately so relay threads see EOF on exit.
    // SAFETY: Parent exclusively owns these write descriptors.
    unsafe {
        if let Some(ow) = out_w_opt {
            libc::close(ow);
        }
        libc::close(err_w);
        libc::close(ctl_w);
    }

    let mut child = match spawn_res {
        Ok(c) => c,
        Err(err) => {
            if let Some(ref rx) = out_done_rx {
                let _ = rx.recv_timeout(PIPE_DRAIN_AFTER_EXIT);
            }
            let _ = err_done_rx.recv_timeout(PIPE_DRAIN_AFTER_EXIT);
            return Err(err);
        }
    };

    let child_pgid = child.id() as libc::pid_t;
    signals.set_active_child_pgid(child_pgid);

    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();

    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        if let Some(ref mut s) = child_stdout {
            pump_reader_to_channel(s, &stdout_tx);
        }
    });
    let (stderr_tx, stderr_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        if let Some(ref mut s) = child_stderr {
            pump_reader_to_channel(s, &stderr_tx);
        }
    });

    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let mut ctl_bytes = Vec::new();
    let mut interrupted_by = None;

    let status = loop {
        if let Some(sig_exit) = signals.termination_exit_code() {
            interrupted_by = Some(sig_exit);
            break signals.terminate_active_child_and_wait(&mut child)?;
        }
        match platform::wait_child_until(&mut child, None, Some(signals.wakeup_fd()))? {
            ChildWaitOutcome::Exited(st) => break st,
            ChildWaitOutcome::SignalWakeup => {
                signals.drain_wakeup();
                continue;
            }
            ChildWaitOutcome::TimedOut => continue,
        }
    };

    signals.clear_active_child_pgid();

    if let Some(sig_exit) = interrupted_by {
        return Ok(ShellOutput {
            stdout: String::new(),
            exit_code: sig_exit.as_i32(),
            anix_out_bytes: Vec::new(),
            ctl_frames: Vec::new(),
            ctl_warnings: Vec::new(),
            interrupted_by: Some(sig_exit),
        });
    }

    let drain_deadline = Instant::now() + PIPE_DRAIN_AFTER_EXIT;
    drain_channel_until(&stdout_rx, &mut stdout_bytes, drain_deadline);
    drain_channel_until(&stderr_rx, &mut stderr_bytes, drain_deadline);
    drain_channel_until(&ctl_rx, &mut ctl_bytes, drain_deadline);
    let anix_out_bytes = if let Some(ref rx) = out_done_rx {
        rx.recv_timeout(drain_deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let _ = err_done_rx.recv_timeout(drain_deadline.saturating_duration_since(Instant::now()));
    relay_active.store(false, Ordering::SeqCst);

    let mut combined = String::from_utf8_lossy(&stdout_bytes).into_owned();
    if !stderr_bytes.is_empty() {
        let err_str = String::from_utf8_lossy(&stderr_bytes);
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&err_str);
    }

    let (ctl_frames, ctl_warnings) = parse_ctl_frames(&ctl_bytes);

    let exit_code = if let Some(code) = status.code() {
        code
    } else if let Some(sig) = status.signal() {
        128 + sig
    } else {
        ExitCode::RuntimeError.as_i32()
    };

    Ok(ShellOutput {
        stdout: combined,
        exit_code,
        anix_out_bytes,
        ctl_frames,
        ctl_warnings,
        interrupted_by,
    })
}

fn pump_reader_to_channel<R: Read>(reader: &mut R, tx: &mpsc::Sender<Vec<u8>>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

fn drain_channel_until(rx: &mpsc::Receiver<Vec<u8>>, dst: &mut Vec<u8>, deadline: Instant) {
    loop {
        let rem = deadline.saturating_duration_since(Instant::now());
        if rem.is_zero() {
            while let Ok(c) = rx.try_recv() {
                dst.extend_from_slice(&c);
            }
            break;
        }
        match rx.recv_timeout(rem) {
            Ok(c) => dst.extend_from_slice(&c),
            Err(mpsc::RecvTimeoutError::Disconnected) | Err(mpsc::RecvTimeoutError::Timeout) => {
                break;
            }
        }
    }
}

fn dup_above_6(fd: RawFd) -> io::Result<RawFd> {
    // SAFETY: `F_DUPFD_CLOEXEC` duplicates `fd` to the lowest unused FD >= 7 with CLOEXEC.
    let new_fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 7) };
    if new_fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(new_fd)
    }
}

fn dup2_inheritable(src: RawFd, dst: RawFd) -> io::Result<()> {
    // SAFETY: `dup2` atomically duplicates `src` onto `dst` with `FD_CLOEXEC` cleared when `src != dst`.
    if unsafe { libc::dup2(src, dst) } < 0 {
        return Err(io::Error::last_os_error());
    }
    if src == dst {
        posix::clear_cloexec(dst)?;
    }
    Ok(())
}

fn parse_ctl_frames(raw: &[u8]) -> (Vec<CtlFrame>, Vec<String>) {
    let mut frames = Vec::new();
    let mut warnings = Vec::new();
    if raw.is_empty() {
        return (frames, warnings);
    }

    for (idx, line) in raw.split(|&b| b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        if line.len() > MAX_CTL_FRAME_BYTES {
            warnings.push(format!(
                "ignored $ANIX_CTL frame #{} exceeding {} bytes ({} bytes)",
                idx + 1,
                MAX_CTL_FRAME_BYTES,
                line.len()
            ));
            continue;
        }
        match std::str::from_utf8(line) {
            Ok(s) => {
                if s.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<CtlFrame>(s) {
                    Ok(frame) => frames.push(frame),
                    Err(err) => warnings.push(format!(
                        "ignored malformed $ANIX_CTL frame #{}: {err}",
                        idx + 1
                    )),
                }
            }
            Err(_) => warnings.push(format!("ignored non-UTF-8 $ANIX_CTL frame #{}", idx + 1)),
        }
    }

    (frames, warnings)
}
