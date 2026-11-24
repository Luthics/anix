//! External FD 0/1/2/3/4..9 contract, `FD_CLOEXEC` hygiene, anonymous pipe relays
//! (`fd 4/5/6` for `$AGENT_OUT`, `$AGENT_ERR`, `$AGENT_CTL`), and Maildir-style
//! `--inbox <DIR>` / `--idle-timeout <SEC>` daemon queue (Section 4).

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const PIPE_DRAIN_AFTER_EXIT: Duration = Duration::from_millis(200);

use os_agent_proto::ctl::{CtlFrame, MAX_CTL_FRAME_BYTES};
use os_agent_proto::exit_code::ExitCode;

use crate::platform::{self, DirWatcher, PreparedSandbox, posix};
use crate::signal::SignalState;

/// Drain and immediately close any inherited readable static multi-input FDs (`fd 4..=9`)
/// BEFORE opening any internal file descriptors (`journal.sock`, `signal-hook`, WAL `flock`, etc.)
/// so internal FDs can never collide with inherited `fd 4..=9` (Section 4.1).
pub fn drain_inherited_extra_fds() -> Vec<(RawFd, String)> {
    let mut extras = Vec::new();
    for fd in 4..=9 {
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

/// Assemble initial user input from `<SCRIPT_ARGS>`, `fd 0` (`stdin`), and pre-drained `fd 4..=9`
/// per Section 4.1.
///
/// When `skip_tty_stdin` is true (used in `--inbox` mode so interactive startup doesn't block
/// waiting for Ctrl+D before listening on `new/`), `fd 0` is skipped whenever `isatty(0)` is true.
pub fn assemble_initial_inputs(
    script_args: &[String],
    extra_fds: &[(RawFd, String)],
    skip_tty_stdin: bool,
    signals: &SignalState,
) -> io::Result<String> {
    let mut sections: Vec<String> = Vec::new();

    if !script_args.is_empty() {
        sections.push(format!("<args>{}</args>", script_args.join(" ")));
    }

    // Section 4.1: If `isatty(0)` is true AND `<SCRIPT_ARGS>` were provided (or `--inbox` startup),
    // NEVER block waiting for stdin EOF.
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

    Ok(sections.join("\n"))
}

fn read_fd_interruptible(fd: RawFd, signals: &SignalState) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        if signals.termination_exit_code().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "interrupted by signal while reading stdin",
            ));
        }
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: Valid single-element pollfd slice.
        let rc = unsafe { libc::poll(&mut pfd, 1, 50) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if rc == 0 {
            continue;
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

/// Output captured from a single `msh -c "<cmd>"` invocation.
#[derive(Debug)]
pub struct MshOutput {
    pub stdout: String,
    pub exit_code: i32,
    pub ctl_frames: Vec<CtlFrame>,
    pub ctl_warnings: Vec<String>,
    pub interrupted_by: Option<ExitCode>,
}

/// Spawn `msh -c <cmd>` with anonymous pipe relays for `fd 4` (`$AGENT_OUT` -> external `fd 1`),
/// `fd 5` (`$AGENT_ERR` -> external `fd 2`), and `fd 6` (`$AGENT_CTL` -> JSONL control frames).
#[allow(clippy::too_many_arguments)]
pub fn run_msh_command(
    shell_bin: &Path,
    tools_dir: &Path,
    cmd_str: &str,
    cwd: &Path,
    msh_timeout_sec: u64,
    sandbox: Option<&PreparedSandbox>,
    signals: &SignalState,
) -> io::Result<MshOutput> {
    let (out_r, out_w) = posix::create_pipe_cloexec()?;
    let (err_r, err_w) = posix::create_pipe_cloexec()?;
    let (ctl_r, ctl_w) = posix::create_pipe_cloexec()?;

    // On Linux, opening `/dev/fd/4` (`/proc/self/fd/4`) with `O_RDONLY` (e.g. `wc -l "$AGENT_OUT"`
    // or `cat "$AGENT_OUT"`) normally opens the pipe's read end and blocks until timeout instead of
    // returning EACCES like macOS `fdescfs`. Marking the pipe inode write-only (`0200`) AFTER `out_r`
    // and `err_r` are already opened makes child `O_RDONLY` opens fail immediately with EACCES.
    #[cfg(target_os = "linux")]
    unsafe {
        let _ = libc::fchmod(out_w, 0o200);
        let _ = libc::fchmod(err_w, 0o200);
    }

    // SAFETY: Pipe read ends are exclusively owned by their respective relay threads.
    let mut out_reader = unsafe { File::from_raw_fd(out_r) };
    let mut err_reader = unsafe { File::from_raw_fd(err_r) };
    let mut ctl_reader = unsafe { File::from_raw_fd(ctl_r) };

    // Relay `$AGENT_OUT` raw bytes -> external `fd 1` (stdout) without O_TRUNC risk.
    let (out_done_tx, out_done_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let mut stdout = io::stdout();
        let _ = io::copy(&mut out_reader, &mut stdout);
        let _ = stdout.flush();
        let _ = out_done_tx.send(());
    });

    // Relay `$AGENT_ERR` raw bytes -> external `fd 2` (stderr) in both auto & explicit modes.
    let (err_done_tx, err_done_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let mut stderr = io::stderr();
        let _ = io::copy(&mut err_reader, &mut stderr);
        let _ = stderr.flush();
        let _ = err_done_tx.send(());
    });

    // Collect `$AGENT_CTL` chunks over a channel so detached grandchildren can never deadlock.
    let (ctl_tx, ctl_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        pump_reader_to_channel(&mut ctl_reader, &ctl_tx);
    });

    // Build child PATH with `~/.local/libexec/agent/tools/` prepended (Section 2).
    let existing_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path_entries = vec![tools_dir.to_path_buf()];
    path_entries.extend(std::env::split_paths(&existing_path));
    let new_path = std::env::join_paths(path_entries).unwrap_or(existing_path);

    let parent_pid = unsafe { libc::getpid() };
    let sandbox_cloned = sandbox.cloned();

    let mut command = Command::new(shell_bin);
    command
        .arg("-c")
        .arg(cmd_str)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Section 3.4: Prevent nested child agents from inheriting parent WAL file lock.
        .env_remove("AGENT_CONTEXT")
        .env("PATH", new_path)
        .env("AGENT_OUT", "/dev/fd/4")
        .env("AGENT_ERR", "/dev/fd/5")
        .env("AGENT_CTL", "/dev/fd/6")
        .env("MSH_TIMEOUT_SEC", msh_timeout_sec.to_string());

    // SAFETY: `pre_exec` uses only async-signal-safe syscalls to isolate PGID, bind FDs 4/5/6,
    // set Linux `PR_SET_PDEATHSIG`, and apply pre-built Landlock rules.
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

            // Safely relocate pipe write FDs to >= 7 first so `dup2` onto 4, 5, 6 never clobbers.
            let tmp_out = dup_above_6(out_w)?;
            let tmp_err = dup_above_6(err_w)?;
            let tmp_ctl = dup_above_6(ctl_w)?;

            dup2_inheritable(tmp_out, 4)?;
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
        libc::close(out_w);
        libc::close(err_w);
        libc::close(ctl_w);
    }

    let mut child = match spawn_res {
        Ok(c) => c,
        Err(err) => {
            let _ = out_done_rx.recv_timeout(PIPE_DRAIN_AFTER_EXIT);
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
        while let Ok(c) = stdout_rx.try_recv() {
            stdout_bytes.extend_from_slice(&c);
        }
        while let Ok(c) = stderr_rx.try_recv() {
            stderr_bytes.extend_from_slice(&c);
        }
        while let Ok(c) = ctl_rx.try_recv() {
            ctl_bytes.extend_from_slice(&c);
        }
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if let Some(sig_exit) = signals.termination_exit_code() {
            interrupted_by = Some(sig_exit);
            break signals.terminate_active_child_and_wait(&mut child)?;
        }
        thread::sleep(Duration::from_millis(15));
    };

    signals.clear_active_child_pgid();

    if let Some(sig_exit) = interrupted_by {
        // Immediate return on SIGINT/SIGTERM without waiting on any orphaned grandchild pipes.
        return Ok(MshOutput {
            stdout: String::new(),
            exit_code: sig_exit.as_i32(),
            ctl_frames: Vec::new(),
            ctl_warnings: Vec::new(),
            interrupted_by: Some(sig_exit),
        });
    }

    // Bounded post-exit drain captures 100% of flushed bytes in <1ms normally while preventing
    // infinite hangs if a detached grandchild held fd 1/2/4/5/6 open after timeout.
    let drain_deadline = Instant::now() + PIPE_DRAIN_AFTER_EXIT;
    drain_channel_until(&stdout_rx, &mut stdout_bytes, drain_deadline);
    drain_channel_until(&stderr_rx, &mut stderr_bytes, drain_deadline);
    drain_channel_until(&ctl_rx, &mut ctl_bytes, drain_deadline);
    let _ = out_done_rx.recv_timeout(drain_deadline.saturating_duration_since(Instant::now()));
    let _ = err_done_rx.recv_timeout(drain_deadline.saturating_duration_since(Instant::now()));

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

    Ok(MshOutput {
        stdout: combined,
        exit_code,
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
                "ignored $AGENT_CTL frame #{} exceeding {} bytes ({} bytes)",
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
                        "ignored malformed $AGENT_CTL frame #{}: {err}",
                        idx + 1
                    )),
                }
            }
            Err(_) => warnings.push(format!("ignored non-UTF-8 $AGENT_CTL frame #{}", idx + 1)),
        }
    }

    (frames, warnings)
}

/// An atomic message claimed from `<inbox>/new/<name>` into `<inbox>/cur/<name>` (Section 4.2).
#[derive(Debug, Clone)]
pub struct InboxMessage {
    pub name: String,
    pub cur_path: PathBuf,
    pub formatted_input: String,
}

/// Maildir-style atomic directory queue (`tmp/`, `new/`, `cur/`, `done/`, mode `0700`)
/// watched via Linux `inotify` / macOS `kqueue` with 200ms poll fallback (Section 4.2).
#[derive(Debug)]
pub struct InboxQueue {
    new_dir: PathBuf,
    cur_dir: PathBuf,
    done_dir: PathBuf,
    watcher: Option<DirWatcher>,
}

impl InboxQueue {
    /// Ensure `<dir>` and `tmp/`, `new/`, `cur/`, `done/` exist with mode `0700` and attach watcher.
    pub fn open(dir: &Path) -> io::Result<Self> {
        let tmp_dir = dir.join("tmp");
        let new_dir = dir.join("new");
        let cur_dir = dir.join("cur");
        let done_dir = dir.join("done");

        posix::ensure_dir_mode(dir, 0o700)?;
        posix::ensure_dir_mode(&tmp_dir, 0o700)?;
        posix::ensure_dir_mode(&new_dir, 0o700)?;
        posix::ensure_dir_mode(&cur_dir, 0o700)?;
        posix::ensure_dir_mode(&done_dir, 0o700)?;

        let watcher = DirWatcher::watch(&new_dir).ok();
        Ok(Self {
            new_dir,
            cur_dir,
            done_dir,
            watcher,
        })
    }

    /// Wait for the next ASCII-sorted message in `new/`, atomically rename it to `cur/`,
    /// and return its wrapped payload. Returns `Ok(None)` on `--idle-timeout` expiry or
    /// `SIGINT`/`SIGTERM`.
    pub fn next_message(
        &mut self,
        idle_timeout: Option<Duration>,
        signals: &SignalState,
        on_snapshot: &mut impl FnMut(),
    ) -> io::Result<Option<InboxMessage>> {
        let idle_start = Instant::now();

        loop {
            if signals.termination_exit_code().is_some() {
                return Ok(None);
            }
            if signals.take_snapshot_request() {
                on_snapshot();
            }

            if let Some(msg) = self.try_claim_next()? {
                return Ok(Some(msg));
            }

            if let Some(limit) = idle_timeout
                && idle_start.elapsed() >= limit
            {
                return Ok(None);
            }

            // Wait up to 200ms (or remaining idle budget) via inotify/kqueue or sleep fallback.
            let wait_ms = match idle_timeout {
                Some(limit) => {
                    let rem = limit.saturating_sub(idle_start.elapsed());
                    (rem.as_millis() as i32).clamp(1, 200)
                }
                None => 200,
            };

            if let Some(ref watcher) = self.watcher {
                let _ = watcher.wait(wait_ms);
            } else {
                thread::sleep(Duration::from_millis(wait_ms as u64));
            }
        }
    }

    /// Move a completed message atomically from `cur/<name>` to `done/<name>`.
    pub fn mark_done(&self, msg: &InboxMessage) -> io::Result<()> {
        let dest = self.done_dir.join(&msg.name);
        fs::rename(&msg.cur_path, dest)
    }

    fn try_claim_next(&self) -> io::Result<Option<InboxMessage>> {
        let mut names: Vec<String> = Vec::new();
        for entry in fs::read_dir(&self.new_dir)? {
            let Ok(entry) = entry else { continue };
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            names.push(name);
        }

        // Strict ASCII lexicographical order (Section 4.2.3).
        names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        for name in names {
            let src = self.new_dir.join(&name);
            let cur = self.cur_dir.join(&name);
            match fs::rename(&src, &cur) {
                Ok(()) => {
                    let bytes = fs::read(&cur)?;
                    let body = String::from_utf8_lossy(&bytes);
                    let formatted_input = format!("<input source=\"{name}\">\n{body}\n</input>");
                    return Ok(Some(InboxMessage {
                        name,
                        cur_path: cur,
                        formatted_input,
                    }));
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            }
        }

        Ok(None)
    }
}
