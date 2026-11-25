//! Isolated process-group command execution, anti-hang env injection, and
//! timeout / signal reaping (Section 7.1.1 & 7.1.5).

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anix_proto::exit_code::ExitCode;
use thiserror::Error;

pub const DEFAULT_TIMEOUT_SEC: u64 = 120;
// Shorter than `anix`'s 3s child grace so `msh` always escalates SIGKILL to `/bin/sh`
// cleanly before `anix` ever escalates SIGKILL to `msh`.
const KILL_GRACE_PERIOD: Duration = Duration::from_secs(2);
const PIPE_DRAIN_AFTER_EXIT: Duration = Duration::from_millis(200);

/// Anti-hang environment variables forcefully injected into every `/bin/sh -c` child.
const ANTI_HANG_ENV: &[(&str, &str)] = &[
    ("NONINTERACTIVE", "1"),
    ("DEBIAN_FRONTEND", "noninteractive"),
    ("PAGER", "cat"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("TERM", "dumb"),
    ("NO_COLOR", "1"),
];

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("failed to create capture pipe: {0}")]
    Pipe(#[source] io::Error),
    #[error("failed to spawn /bin/sh: {0}")]
    Spawn(#[source] io::Error),
    #[error("failed to wait for child process: {0}")]
    Wait(#[source] io::Error),
}

/// Raw execution outcome before binary/ANSI/fold post-processing.
#[derive(Debug)]
pub struct ExecResult {
    pub raw_output: Vec<u8>,
    pub exit_code: i32,
    pub timed_out: bool,
    pub timeout_sec: u64,
}

/// Resolve command timeout in seconds from `MSH_TIMEOUT_SEC` (default 120).
pub fn timeout_from_env() -> u64 {
    std::env::var("MSH_TIMEOUT_SEC")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_TIMEOUT_SEC)
}

/// Execute `command` via `/bin/sh -c` in a dedicated process group with
/// merged stdout/stderr capture, `/dev/null` stdin, anti-hang env, and timeout.
pub fn run_command(command: &str, timeout_sec: u64) -> Result<ExecResult, ExecError> {
    let terminated = Arc::new(AtomicBool::new(false));
    let interrupted = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminated));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted));

    let (read_fd, write_fd) = create_pipe_cloexec().map_err(ExecError::Pipe)?;
    // Duplicate write_fd for stderr so stdout and stderr share a single merged pipe.
    let write_fd_err = dup_cloexec(write_fd).map_err(|e| {
        // SAFETY: Both descriptors are valid open pipe ends owned by us.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
        ExecError::Pipe(e)
    })?;

    // SAFETY: We just created `read_fd`, `write_fd`, and `write_fd_err` and own them exclusively.
    let mut reader = unsafe { File::from_raw_fd(read_fd) };
    let stdout_stdio = unsafe { Stdio::from_raw_fd(write_fd) };
    let stderr_stdio = unsafe { Stdio::from_raw_fd(write_fd_err) };

    let parent_pid = unsafe { libc::getpid() };

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(stdout_stdio)
        .stderr(stderr_stdio);

    for &(k, v) in ANTI_HANG_ENV {
        cmd.env(k, v);
    }

    // SAFETY: `pre_exec` executes only async-signal-safe syscalls (`setsid`/`setpgid`,
    // `prctl`, `getppid`, `_exit`) between `fork` and `execve`.
    unsafe {
        cmd.pre_exec(move || {
            // Isolate into a new session / process group so signal reaping covers the whole tree.
            if libc::setsid() == -1 && libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }

            #[cfg(target_os = "linux")]
            {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != parent_pid {
                    libc::_exit(1);
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = parent_pid;

            Ok(())
        });
    }

    let mut child = cmd.spawn().map_err(ExecError::Spawn)?;
    // Drop `cmd` immediately so the parent closes its copies of the capture pipe write ends,
    // allowing the reader thread to observe EOF cleanly when the child exits.
    drop(cmd);
    let child_pgid = child.id() as libc::pid_t;

    // Drain merged stdout+stderr continuously over a channel so detached grandchildren
    // holding pipe write ends can never deadlock `msh` after exit, timeout, or signal.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(chunk[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_sec);
    let mut timed_out = false;
    let mut got_sig: Option<i32> = None;
    let mut raw_output: Vec<u8> = Vec::new();

    let status: ExitStatus = loop {
        while let Ok(chunk) = rx.try_recv() {
            raw_output.extend_from_slice(&chunk);
        }
        if let Some(status) = child.try_wait().map_err(ExecError::Wait)? {
            break status;
        }
        if interrupted.load(Ordering::Relaxed) {
            got_sig = Some(ExitCode::SigInt.as_i32());
            break terminate_process_group(child_pgid, &mut child)?;
        }
        if terminated.load(Ordering::Relaxed) {
            got_sig = Some(ExitCode::SigTerm.as_i32());
            break terminate_process_group(child_pgid, &mut child)?;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            break terminate_process_group(child_pgid, &mut child)?;
        }
        thread::sleep(Duration::from_millis(15));
    };

    // Bounded post-exit drain: captures all output flushed before exit without ever hanging
    // if a detached `setsid()` grandchild still holds stdout/stderr open.
    let drain_deadline = Instant::now() + PIPE_DRAIN_AFTER_EXIT;
    loop {
        let rem = drain_deadline.saturating_duration_since(Instant::now());
        if rem.is_zero() {
            break;
        }
        match rx.recv_timeout(rem) {
            Ok(chunk) => raw_output.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Disconnected) | Err(mpsc::RecvTimeoutError::Timeout) => {
                break;
            }
        }
    }

    let exit_code = if timed_out {
        ExitCode::TimeoutOrBudget.as_i32()
    } else if let Some(sig_exit) = got_sig {
        sig_exit
    } else if let Some(code) = status.code() {
        code
    } else if let Some(sig) = status.signal() {
        128 + sig
    } else {
        ExitCode::RuntimeError.as_i32()
    };

    Ok(ExecResult {
        raw_output,
        exit_code,
        timed_out,
        timeout_sec,
    })
}

fn terminate_process_group(
    pgid: libc::pid_t,
    child: &mut std::process::Child,
) -> Result<ExitStatus, ExecError> {
    if pgid > 0 {
        // SAFETY: Sending SIGTERM to the isolated child process group `-pgid`.
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
    }
    let grace_end = Instant::now() + KILL_GRACE_PERIOD;
    let mut reaped_status: Option<ExitStatus> = None;
    while Instant::now() < grace_end {
        if reaped_status.is_none()
            && let Some(st) = child.try_wait().map_err(ExecError::Wait)?
        {
            reaped_status = Some(st);
        }
        if let Some(st) = reaped_status {
            // Re-check if any background members remain in the process group.
            // SAFETY: Signal 0 only probes process group existence.
            if pgid <= 0 || unsafe { libc::kill(-pgid, 0) } == -1 {
                return Ok(st);
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    if pgid > 0 {
        // SAFETY: Escalate to SIGKILL for the entire isolated process group.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    match reaped_status {
        Some(st) => Ok(st),
        None => child.wait().map_err(ExecError::Wait),
    }
}

fn create_pipe_cloexec() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for `pipe(2)`.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for &fd in &fds {
        // SAFETY: Newly created pipe FDs; setting FD_CLOEXEC prevents leaking into children.
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            let err = io::Error::last_os_error();
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            return Err(err);
        }
    }
    Ok((fds[0], fds[1]))
}

fn dup_cloexec(fd: RawFd) -> io::Result<RawFd> {
    // SAFETY: `F_DUPFD_CLOEXEC` duplicates `fd` atomically with `FD_CLOEXEC`.
    let new_fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
    if new_fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(new_fd)
    }
}
