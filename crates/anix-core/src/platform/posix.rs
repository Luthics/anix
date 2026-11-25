//! Shared POSIX helpers (`fcntl`, `flock`, `/dev/fd`, process groups, atomic `0600` files).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

/// Check whether `/dev/fd` is mounted and accessible (Section 10.3).
pub fn dev_fd_available() -> bool {
    Path::new("/dev/fd").is_dir()
}

/// Probe via `fcntl(fd, F_GETFL)` whether `fd` is open and writable (`O_WRONLY` or `O_RDWR`).
pub fn fd_is_writable(fd: RawFd) -> bool {
    // SAFETY: `F_GETFL` is read-only and returns -1/EBADF if `fd` is closed.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return false;
    }
    let mode = flags & libc::O_ACCMODE;
    mode == libc::O_WRONLY || mode == libc::O_RDWR
}

/// Probe via `fcntl(fd, F_GETFL)` whether `fd` is open and readable (`O_RDONLY` or `O_RDWR`).
pub fn fd_is_readable(fd: RawFd) -> bool {
    // SAFETY: `F_GETFL` is read-only and returns -1/EBADF if `fd` is closed.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return false;
    }
    let mode = flags & libc::O_ACCMODE;
    mode == libc::O_RDONLY || mode == libc::O_RDWR
}

/// Set `FD_CLOEXEC` on an open file descriptor.
pub fn set_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: Standard `fcntl(F_GETFD/F_SETFD)` on an open descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Clear `FD_CLOEXEC` on an open file descriptor (for explicitly inherited child FDs).
pub fn clear_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: Standard `fcntl(F_GETFD/F_SETFD)` on an open descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Wrapper around `isatty(3)`.
pub fn isatty(fd: RawFd) -> bool {
    // SAFETY: `isatty` is safe for any integer descriptor.
    unsafe { libc::isatty(fd) == 1 }
}

/// Ignore `SIGPIPE` process-wide so broken pipes on `fd 3` / `journal.sock` / child pipes
/// return `EPIPE` instead of terminating `anix`.
pub fn ignore_sigpipe() {
    // SAFETY: Setting `SIGPIPE` disposition to `SIG_IGN` is standard POSIX hygiene.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

/// Set `O_NONBLOCK` on an open file descriptor.
pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: Standard `fcntl(F_GETFL/F_SETFL)` on an open descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Convert an optional deadline into exact `poll(2)`/`epoll_wait(2)` timeout milliseconds
/// (`-1` for infinite wait, `0` when already expired).
pub fn deadline_to_poll_ms(deadline: Option<Instant>) -> libc::c_int {
    match deadline {
        None => -1,
        Some(dl) => {
            let rem = dl.saturating_duration_since(Instant::now());
            if rem.is_zero() {
                0
            } else {
                // Round up sub-millisecond remainders to 1ms to avoid busy spin before deadline.
                let ms = rem.as_millis();
                if ms == 0 {
                    1
                } else {
                    ms.min(libc::c_int::MAX as u128) as libc::c_int
                }
            }
        }
    }
}

/// Outcome of polling a target FD alongside a signal self-pipe FD with an exact deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollFdOutcome {
    FdReady,
    SignalWakeup,
    TimedOut,
}

/// Event-driven wait until `fd` is readable (or HUP/ERR), `sig_fd` is readable, or `deadline` expires.
pub fn poll_readable_or_signal(
    fd: RawFd,
    sig_fd: RawFd,
    deadline: Option<Instant>,
) -> io::Result<PollFdOutcome> {
    loop {
        let timeout_ms = deadline_to_poll_ms(deadline);
        if timeout_ms == 0 {
            return Ok(PollFdOutcome::TimedOut);
        }
        let mut pfds = [
            libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: sig_fd,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: `pfds` points to 2 initialized `pollfd` entries.
        let rc = unsafe { libc::poll(pfds.as_mut_ptr(), 2, timeout_ms) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(PollFdOutcome::SignalWakeup);
            }
            return Err(err);
        }
        if rc == 0 {
            return Ok(PollFdOutcome::TimedOut);
        }
        if pfds[1].revents != 0 {
            return Ok(PollFdOutcome::SignalWakeup);
        }
        if pfds[0].revents != 0 {
            return Ok(PollFdOutcome::FdReady);
        }
    }
}

/// Sleep up to `duration` using kernel `poll(2)` on `sig_fd` (zero fixed-interval polling).
/// Returns `true` if woken early by a signal on `sig_fd`.
pub fn sleep_or_signal(duration: Duration, sig_fd: RawFd) -> bool {
    let deadline = Instant::now() + duration;
    let timeout_ms = deadline_to_poll_ms(Some(deadline));
    if timeout_ms == 0 {
        return false;
    }
    let mut pfd = libc::pollfd {
        fd: sig_fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: Valid single-element pollfd slice.
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if rc > 0 {
        return true;
    }
    if rc == 0 {
        return false;
    }
    io::Error::last_os_error().kind() == io::ErrorKind::Interrupted
}

/// Create an anonymous pipe with `FD_CLOEXEC` set on both ends.
pub fn create_pipe_cloexec() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-int buffer for `pipe(2)`.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if let Err(err) = set_cloexec(fds[0]).and_then(|_| set_cloexec(fds[1])) {
        // SAFETY: Both descriptors are open pipe ends owned by this function.
        unsafe {
            libc::close(fds[0]);
            libc::close(fds[1]);
        }
        return Err(err);
    }
    Ok((fds[0], fds[1]))
}

/// Acquire an exclusive non-blocking `flock(LOCK_EX | LOCK_NB)`.
/// Returns `Ok(true)` if acquired, `Ok(false)` on `EWOULDBLOCK`, or `Err` on other OS error.
pub fn flock_exclusive_nonblocking(fd: RawFd) -> io::Result<bool> {
    // SAFETY: `flock` on a valid open file descriptor.
    let rc = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if rc == 0 {
        Ok(true)
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
            Ok(false)
        } else {
            Err(err)
        }
    }
}

/// Return current effective UID.
pub fn current_uid() -> u32 {
    // SAFETY: `geteuid(2)` cannot fail.
    unsafe { libc::geteuid() }
}

/// Return current parent PID (`ppid`).
pub fn current_ppid() -> u32 {
    // SAFETY: `getppid(2)` cannot fail.
    unsafe { libc::getppid() as u32 }
}

/// Format current UTC time as RFC 3339 (e.g. `2026-11-24T13:30:00.123Z`).
pub fn timestamp_rfc3339_utc() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}

/// Return current Unix epoch time in milliseconds.
pub fn timestamp_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Ensure a directory exists with exact POSIX permissions (e.g. `0o700`).
pub fn ensure_dir_mode(path: &Path, mode: u32) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

/// Atomically write bytes to `dest` with mode `0600` (temp sibling + rename).
pub fn write_atomic_0600(dest: &Path, content: &[u8]) -> io::Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("/tmp"));
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
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

/// Create a brand-new `0600` file at `/tmp/<prefix>_<uuid>.<ext>`.
pub fn create_tmp_0600(prefix: &str, ext: &str) -> io::Result<(PathBuf, File)> {
    let path = PathBuf::from(format!("/tmp/{prefix}_{}.{ext}", Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    Ok((path, file))
}

/// Send `SIGTERM` to an isolated child process group (`-pgid`), wait via kernel events
/// (`pidfd`/`kqueue`, zero polling) up to `grace` to reap `child`, and escalate to `SIGKILL`.
pub fn terminate_child_process_group(
    child: &mut std::process::Child,
    pgid: libc::pid_t,
    grace: Duration,
) -> io::Result<std::process::ExitStatus> {
    if pgid > 0 {
        // SAFETY: Signalling isolated child process group `-pgid`.
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
    }
    let deadline = Instant::now() + grace;
    let reaped_status = match super::wait_child_until(child, Some(deadline), None)? {
        super::ChildWaitOutcome::Exited(st) => Some(st),
        super::ChildWaitOutcome::TimedOut | super::ChildWaitOutcome::SignalWakeup => {
            child.try_wait()?
        }
    };

    if let Some(st) = reaped_status {
        // Fast path: if no other process survives in `-pgid`, return immediately.
        // SAFETY: Signal 0 probes whether any live member remains in `-pgid`.
        if pgid <= 0 || unsafe { libc::kill(-pgid, 0) } == -1 {
            return Ok(st);
        }
        // Orphaned grandchildren survived after the direct child exited: wait out remaining grace
        // then escalate to `SIGKILL` to avoid leaking background members.
        let rem = deadline.saturating_duration_since(Instant::now());
        if !rem.is_zero() {
            // Bounded wait using `poll` (no busy spin) checking once halfway if needed.
            let step = rem.min(Duration::from_millis(100));
            while Instant::now() < deadline {
                // SAFETY: Signal 0 checks whether any member remains in `-pgid`.
                if unsafe { libc::kill(-pgid, 0) } == -1 {
                    return Ok(st);
                }
                let wait_ms = deadline_to_poll_ms(Some((Instant::now() + step).min(deadline)));
                if wait_ms <= 0 {
                    break;
                }
                // SAFETY: `poll` with null fds / 0 nfds acts as a POSIX millisecond timer.
                unsafe {
                    libc::poll(std::ptr::null_mut(), 0, wait_ms);
                }
            }
        }
        // SAFETY: Escalate to `SIGKILL` for any remaining members of `-pgid`.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
        return Ok(st);
    }

    if pgid > 0 {
        // SAFETY: Escalate to `SIGKILL` for any remaining members of `-pgid`.
        unsafe {
            libc::kill(-pgid, libc::SIGKILL);
        }
    }
    child.wait()
}

/// Send `SIGTERM` to an entire process group (`-pgid`), wait up to `grace`, then `SIGKILL`.
pub fn terminate_process_group(pgid: libc::pid_t, grace: Duration) {
    if pgid <= 0 {
        return;
    }
    // SAFETY: Signalling isolated child process group `-pgid`.
    unsafe {
        libc::kill(-pgid, libc::SIGTERM);
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        // SAFETY: Signal 0 checks whether any process remains in the group.
        if unsafe { libc::kill(-pgid, 0) } == -1 {
            return;
        }
        let step = Duration::from_millis(50);
        let wait_ms = deadline_to_poll_ms(Some((Instant::now() + step).min(deadline)));
        if wait_ms <= 0 {
            break;
        }
        // SAFETY: `poll` with null fds / 0 nfds acts as a POSIX millisecond timer.
        unsafe {
            libc::poll(std::ptr::null_mut(), 0, wait_ms);
        }
    }
    // SAFETY: Escalate to `SIGKILL` for any remaining members of `-pgid`.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

/// Check if a file path is executable (`mode & 0o111 != 0`).
pub fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && (meta.permissions().mode() & 0o111 != 0)
}
