//! Isolated process-group command execution, anti-hang env injection, pure
//! event-driven kernel waiting (`pidfd`/`kqueue` + self-pipe), and yield-to-background
//! / optional hard-timeout reaping (v1.1.2 §§6.3, 7.1.1 & 7.1.5).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::io::{FromRawFd, RawFd};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

use anix_proto::ctl::{CtlFrame, MAX_CTL_FRAME_BYTES};
use anix_proto::exit_code::ExitCode;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_TIMEOUT_SEC: u64 = 120;
// Shorter than `anix`'s 3s child grace so `msh` always escalates SIGKILL to `/bin/sh`
// cleanly before `anix` ever escalates SIGKILL to `msh`.
const KILL_GRACE_PERIOD: Duration = Duration::from_secs(2);
const PREVIEW_READ_TAIL_BYTES: u64 = 64 * 1024;
const MAX_CTL_CMD_CHARS: usize = 512;

/// Outer POSIX shell wrapper that runs `$1` via `/bin/sh -c` and atomically writes
/// its exit status to `$2` (`tmp -> mv`) even after `msh` has yielded to background.
const JOB_WRAPPER: &str = "/bin/sh -c \"$1\" sh; _rc=$?; printf '%s\\n' \"$_rc\" > \"$2.tmp\" && mv -f \"$2.tmp\" \"$2\"; exit \"$_rc\"";

/// Anti-hang environment variables forcefully injected into every `/bin/sh -c` child.
const ANTI_HANG_ENV: &[(&str, &str)] = &[
    ("NONINTERACTIVE", "1"),
    ("DEBIAN_FRONTEND", "noninteractive"),
    ("PAGER", "cat"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("TERM", "dumb"),
    ("NO_COLOR", "1"),
];

static SIG_WAKE_WR: AtomicI32 = AtomicI32::new(-1);
static GOT_SIGINT: AtomicBool = AtomicBool::new(false);
static GOT_SIGTERM: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(sig: libc::c_int) {
    match sig {
        libc::SIGINT => GOT_SIGINT.store(true, Ordering::SeqCst),
        libc::SIGTERM => GOT_SIGTERM.store(true, Ordering::SeqCst),
        _ => {}
    }
    let fd = SIG_WAKE_WR.load(Ordering::Relaxed);
    if fd >= 0 {
        let b = 1u8;
        // SAFETY: `write(2)` on a non-blocking pipe FD is async-signal-safe.
        unsafe {
            let _ = libc::write(fd, (&b as *const u8).cast(), 1);
        }
    }
}

struct SignalWakeup {
    rd: RawFd,
    wr: RawFd,
}

impl SignalWakeup {
    fn install() -> io::Result<Self> {
        let (rd, wr) = create_nonblock_pipe_cloexec()?;
        GOT_SIGINT.store(false, Ordering::SeqCst);
        GOT_SIGTERM.store(false, Ordering::SeqCst);
        SIG_WAKE_WR.store(wr, Ordering::SeqCst);

        // SAFETY: Installing async-signal-safe C signal handlers for SIGINT, SIGTERM, SIGCHLD.
        unsafe {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handle_signal as *const () as usize;
            libc::sigemptyset(&mut sa.sa_mask);
            sa.sa_flags = 0;
            let _ = libc::sigaction(libc::SIGINT, &sa, std::ptr::null_mut());
            let _ = libc::sigaction(libc::SIGTERM, &sa, std::ptr::null_mut());
            let _ = libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());
        }

        Ok(Self { rd, wr })
    }

    fn fd(&self) -> RawFd {
        self.rd
    }

    fn drain(&self) {
        let mut buf = [0u8; 64];
        loop {
            // SAFETY: Non-blocking read from owned self-pipe read end.
            let n = unsafe { libc::read(self.rd, buf.as_mut_ptr().cast(), buf.len()) };
            if n <= 0 {
                break;
            }
        }
    }
}

impl Drop for SignalWakeup {
    fn drop(&mut self) {
        SIG_WAKE_WR.store(-1, Ordering::SeqCst);
        // SAFETY: Owned FDs closed once on drop.
        unsafe {
            libc::close(self.rd);
            libc::close(self.wr);
        }
    }
}

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("failed to create signal/job resources: {0}")]
    Setup(#[source] io::Error),
    #[error("failed to spawn /bin/sh: {0}")]
    Spawn(#[source] io::Error),
    #[error("failed to wait for child process: {0}")]
    Wait(#[source] io::Error),
}

/// Execution policy resolved from environment variables.
#[derive(Debug, Clone)]
pub struct ExecOptions {
    pub timeout_sec: u64,
    pub kill_on_timeout: bool,
    pub jobs_dir: PathBuf,
}

impl ExecOptions {
    pub fn from_env() -> Self {
        let timeout_sec = std::env::var("MSH_TIMEOUT_SEC")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SEC);
        let kill_on_timeout = std::env::var("MSH_KILL_ON_TIMEOUT")
            .ok()
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        let effective_timeout = if kill_on_timeout && timeout_sec == 0 {
            1
        } else {
            timeout_sec
        };
        let jobs_dir = resolve_jobs_dir();
        Self {
            timeout_sec: effective_timeout,
            kill_on_timeout,
            jobs_dir,
        }
    }
}

/// Execution outcome returned to `msh` CLI formatter.
#[derive(Debug)]
pub enum ExecOutcome {
    /// Finished within foreground wait window.
    Completed { raw_output: Vec<u8>, exit_code: i32 },
    /// Exceeded `timeout_sec` with `kill_on_timeout = true` and was terminated.
    TimedOutKilled {
        raw_output: Vec<u8>,
        timeout_sec: u64,
    },
    /// Interrupted by `SIGINT` (130) or `SIGTERM` (143) while waiting in foreground.
    Signaled { raw_output: Vec<u8>, exit_code: i32 },
    /// Exceeded `timeout_sec` with `kill_on_timeout = false` (or `timeout_sec == 0`):
    /// process group continues running in background.
    Backgrounded {
        job_id: String,
        pid: u32,
        pgid: u32,
        log_path: PathBuf,
        exit_file: PathBuf,
        preview_bytes: Vec<u8>,
        waited_sec: u64,
        immediate: bool,
    },
}

/// Resolve `MSH_TIMEOUT_SEC` (accepts `0` for immediate background; defaults to `120`).
pub fn timeout_from_env() -> u64 {
    ExecOptions::from_env().timeout_sec
}

/// Resolve job log/exit directory (`MSH_JOBS_DIR` -> `$XDG_RUNTIME_DIR/anix/jobs` -> `/tmp/anix-<uid>/jobs`).
fn resolve_jobs_dir() -> PathBuf {
    if let Ok(raw) = std::env::var("MSH_JOBS_DIR")
        && !raw.trim().is_empty()
    {
        return PathBuf::from(raw.trim());
    }
    if let Ok(rt) = std::env::var("XDG_RUNTIME_DIR")
        && !rt.trim().is_empty()
    {
        return PathBuf::from(rt.trim()).join("anix/jobs");
    }
    // SAFETY: `getuid(2)` is always safe.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/anix-{uid}/jobs"))
}

fn ensure_jobs_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
    Ok(())
}

/// Execute `command` via `/bin/sh -c` in an isolated `setsid()` process group with
/// file-backed merged stdout/stderr, anti-hang env, pure event-driven wait, and
/// yield-to-background (default) or hard-kill timeout behavior.
pub fn run_command_with_options(
    command: &str,
    opts: &ExecOptions,
) -> Result<ExecOutcome, ExecError> {
    let sig = SignalWakeup::install().map_err(ExecError::Setup)?;
    ensure_jobs_dir(&opts.jobs_dir).map_err(ExecError::Setup)?;

    let job_id = format!("j_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let log_path = opts.jobs_dir.join(format!("{job_id}.log"));
    let exit_file = opts.jobs_dir.join(format!("{job_id}.exit"));
    let exit_tmp = opts.jobs_dir.join(format!("{job_id}.exit.tmp"));

    let log_file = OpenOptions::new()
        .create_new(true)
        .read(true)
        .append(true)
        .mode(0o600)
        .open(&log_path)
        .map_err(ExecError::Setup)?;

    let log_fd = std::os::unix::io::AsRawFd::as_raw_fd(&log_file);
    let stdout_fd = dup_cloexec(log_fd).map_err(ExecError::Setup)?;
    let stderr_fd = dup_cloexec(log_fd).map_err(|e| {
        // SAFETY: `stdout_fd` is owned by us.
        unsafe { libc::close(stdout_fd) };
        ExecError::Setup(e)
    })?;
    drop(log_file);

    // SAFETY: `stdout_fd` and `stderr_fd` are newly duplicated descriptors owned exclusively by us.
    let stdout_stdio = unsafe { Stdio::from_raw_fd(stdout_fd) };
    let stderr_stdio = unsafe { Stdio::from_raw_fd(stderr_fd) };

    // SAFETY: `getpid(2)` is always safe.
    let parent_pid = unsafe { libc::getpid() };
    let immediate_bg = opts.timeout_sec == 0 && !opts.kill_on_timeout;
    let bind_pdeathsig = opts.kill_on_timeout;

    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(JOB_WRAPPER)
        .arg("sh")
        .arg(command)
        .arg(&exit_file)
        .stdin(Stdio::null())
        .stdout(stdout_stdio)
        .stderr(stderr_stdio);

    for &(k, v) in ANTI_HANG_ENV {
        cmd.env(k, v);
    }

    if immediate_bg {
        // Immediate background launches MUST NOT touch the parent turn's `$ANIX_OUT`/`$ANIX_ERR`/`$ANIX_CTL`.
        cmd.env_remove("ANIX_OUT")
            .env_remove("ANIX_ERR")
            .env_remove("ANIX_CTL");
    }

    // SAFETY: `pre_exec` executes only async-signal-safe syscalls (`setsid`/`setpgid`,
    // `close`, `prctl`, `getppid`, `_exit`) between `fork` and `execve`.
    unsafe {
        cmd.pre_exec(move || {
            // Isolate into a new session / process group so signal reaping or backgrounding
            // operates cleanly on the entire command tree.
            if libc::setsid() == -1 && libc::setpgid(0, 0) == -1 {
                return Err(io::Error::last_os_error());
            }

            if immediate_bg {
                let _ = libc::close(4);
                let _ = libc::close(5);
                let _ = libc::close(6);
            }

            #[cfg(target_os = "linux")]
            if bind_pdeathsig {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) == -1 {
                    return Err(io::Error::last_os_error());
                }
                if libc::getppid() != parent_pid {
                    libc::_exit(1);
                }
            }
            #[cfg(not(target_os = "linux"))]
            let _ = (parent_pid, bind_pdeathsig);

            Ok(())
        });
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(err) => {
            let _ = fs::remove_file(&log_path);
            return Err(ExecError::Spawn(err));
        }
    };
    drop(cmd);

    let child_pid = child.id();
    let child_pgid = child_pid as libc::pid_t;

    if immediate_bg {
        emit_bg_job_ctl(
            &job_id, child_pid, child_pid, command, &log_path, &exit_file,
        );
        return Ok(ExecOutcome::Backgrounded {
            job_id,
            pid: child_pid,
            pgid: child_pid,
            log_path,
            exit_file,
            preview_bytes: Vec::new(),
            waited_sec: 0,
            immediate: true,
        });
    }

    let deadline = Instant::now() + Duration::from_secs(opts.timeout_sec);
    let mut timed_out = false;
    let mut got_sig: Option<i32> = None;

    let final_status: Option<ExitStatus> = loop {
        if let Some(st) = child.try_wait().map_err(ExecError::Wait)? {
            break Some(st);
        }
        if GOT_SIGINT.load(Ordering::SeqCst) {
            got_sig = Some(ExitCode::SigInt.as_i32());
            break Some(terminate_process_group(child_pgid, &mut child, sig.fd())?);
        }
        if GOT_SIGTERM.load(Ordering::SeqCst) {
            got_sig = Some(ExitCode::SigTerm.as_i32());
            break Some(terminate_process_group(child_pgid, &mut child, sig.fd())?);
        }
        if Instant::now() >= deadline {
            if let Some(st) = child.try_wait().map_err(ExecError::Wait)? {
                break Some(st);
            }
            timed_out = true;
            if opts.kill_on_timeout {
                break Some(terminate_process_group(child_pgid, &mut child, sig.fd())?);
            } else {
                break None;
            }
        }

        match wait_child_until(&mut child, Some(deadline), sig.fd()).map_err(ExecError::Wait)? {
            WaitOutcome::Exited(st) => break Some(st),
            WaitOutcome::SignalWakeup => {
                sig.drain();
                continue;
            }
            WaitOutcome::TimedOut => continue,
        }
    };

    if timed_out && !opts.kill_on_timeout {
        let preview_bytes = read_file_tail(&log_path, PREVIEW_READ_TAIL_BYTES).unwrap_or_default();
        emit_bg_job_ctl(
            &job_id, child_pid, child_pid, command, &log_path, &exit_file,
        );
        return Ok(ExecOutcome::Backgrounded {
            job_id,
            pid: child_pid,
            pgid: child_pid,
            log_path,
            exit_file,
            preview_bytes,
            waited_sec: opts.timeout_sec,
            immediate: false,
        });
    }

    let raw_output = fs::read(&log_path).unwrap_or_default();
    let _ = fs::remove_file(&log_path);
    let _ = fs::remove_file(&exit_file);
    let _ = fs::remove_file(&exit_tmp);

    if timed_out {
        return Ok(ExecOutcome::TimedOutKilled {
            raw_output,
            timeout_sec: opts.timeout_sec,
        });
    }
    if let Some(sig_exit) = got_sig {
        return Ok(ExecOutcome::Signaled {
            raw_output,
            exit_code: sig_exit,
        });
    }

    let status = final_status.expect("completed child always has ExitStatus");
    let exit_code = if let Some(code) = status.code() {
        code
    } else if let Some(s) = status.signal() {
        128 + s
    } else {
        ExitCode::RuntimeError.as_i32()
    };

    Ok(ExecOutcome::Completed {
        raw_output,
        exit_code,
    })
}

/// Convenience wrapper preserving simple library call signature.
pub fn run_command(command: &str, timeout_sec: u64) -> Result<ExecOutcome, ExecError> {
    let mut opts = ExecOptions::from_env();
    opts.timeout_sec = if opts.kill_on_timeout && timeout_sec == 0 {
        1
    } else {
        timeout_sec
    };
    run_command_with_options(command, &opts)
}

/// Best-effort emit `CtlFrame::BgJob` on `$ANIX_CTL` (`fd 6`) if running inside `anix`.
fn emit_bg_job_ctl(
    job_id: &str,
    pid: u32,
    pgid: u32,
    cmd: &str,
    log_path: &Path,
    exit_file: &Path,
) {
    let Ok(ctl_path) = std::env::var("ANIX_CTL") else {
        return;
    };
    if ctl_path.trim().is_empty() {
        return;
    }

    let mut short_cmd = truncate_chars(cmd, MAX_CTL_CMD_CHARS);
    // Keep total JSON frame well within the 64 KiB `$ANIX_CTL` limit.
    let mut frame = CtlFrame::BgJob {
        job_id: job_id.to_owned(),
        pid,
        pgid,
        cmd: short_cmd.clone(),
        log_path: log_path.display().to_string(),
        exit_file: exit_file.display().to_string(),
    };
    let Ok(mut line) = serde_json::to_string(&frame) else {
        return;
    };
    if line.len() + 1 > MAX_CTL_FRAME_BYTES {
        short_cmd = truncate_chars(cmd, 64);
        frame = CtlFrame::BgJob {
            job_id: job_id.to_owned(),
            pid,
            pgid,
            cmd: short_cmd,
            log_path: log_path.display().to_string(),
            exit_file: exit_file.display().to_string(),
        };
        let Ok(smaller) = serde_json::to_string(&frame) else {
            return;
        };
        line = smaller;
        if line.len() + 1 > MAX_CTL_FRAME_BYTES {
            return;
        }
    }
    line.push('\n');

    if let Ok(mut f) = OpenOptions::new().write(true).open(&ctl_path) {
        let _ = f.write_all(line.as_bytes());
        let _ = f.flush();
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in s.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

fn read_file_tail(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len > max_bytes {
        file.seek(SeekFrom::Start(len - max_bytes))?;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf)
}

#[derive(Debug)]
enum WaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    SignalWakeup,
}

fn deadline_to_poll_ms(deadline: Option<Instant>) -> libc::c_int {
    match deadline {
        None => -1,
        Some(dl) => {
            let rem = dl.saturating_duration_since(Instant::now());
            if rem.is_zero() {
                0
            } else {
                let ms = rem.as_millis().max(1);
                ms.min(libc::c_int::MAX as u128) as libc::c_int
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: RawFd,
) -> io::Result<WaitOutcome> {
    if let Some(st) = child.try_wait()? {
        return Ok(WaitOutcome::Exited(st));
    }
    let pid = child.id() as libc::pid_t;
    // SAFETY: `pidfd_open(pid, 0)` returns a pollable FD that becomes readable upon exit.
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };
    if pidfd < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(WaitOutcome::Exited(child.wait()?));
        }
        return wait_child_poll_sigfd(child, deadline, sig_fd);
    }
    let pidfd = pidfd as RawFd;
    let _ = set_cloexec(pidfd);

    // SAFETY: `epoll_create1(EPOLL_CLOEXEC)` creates an isolated kernel epoll instance.
    let epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
    if epfd < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(pidfd) };
        return Err(err);
    }

    let mut ev_pid = libc::epoll_event {
        events: libc::EPOLLIN as u32,
        u64: 1,
    };
    let mut ev_sig = libc::epoll_event {
        events: libc::EPOLLIN as u32,
        u64: 2,
    };
    if unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, pidfd, &mut ev_pid) } < 0 {
        let err = io::Error::last_os_error();
        unsafe {
            libc::close(epfd);
            libc::close(pidfd);
        }
        return Err(err);
    }
    let _ = unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, sig_fd, &mut ev_sig) };

    let outcome = loop {
        if let Some(st) = child.try_wait()? {
            break Ok(WaitOutcome::Exited(st));
        }
        let timeout_ms = deadline_to_poll_ms(deadline);
        if timeout_ms == 0 {
            break Ok(WaitOutcome::TimedOut);
        }
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 2];
        // SAFETY: Valid writable array of 2 `epoll_event` entries.
        let n = unsafe { libc::epoll_wait(epfd, events.as_mut_ptr(), 2, timeout_ms) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                if let Some(st) = child.try_wait()? {
                    break Ok(WaitOutcome::Exited(st));
                }
                break Ok(WaitOutcome::SignalWakeup);
            }
            break Err(err);
        }
        if n == 0 {
            if let Some(st) = child.try_wait()? {
                break Ok(WaitOutcome::Exited(st));
            }
            break Ok(WaitOutcome::TimedOut);
        }
        if let Some(st) = child.try_wait()? {
            break Ok(WaitOutcome::Exited(st));
        }
        if events[..n as usize].iter().any(|e| e.u64 == 2) {
            break Ok(WaitOutcome::SignalWakeup);
        }
    };

    // SAFETY: Close kernel handles unconditionally.
    unsafe {
        libc::close(epfd);
        libc::close(pidfd);
    }
    outcome
}

#[cfg(target_os = "macos")]
fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: RawFd,
) -> io::Result<WaitOutcome> {
    if let Some(st) = child.try_wait()? {
        return Ok(WaitOutcome::Exited(st));
    }

    // SAFETY: `kqueue(2)` creates an isolated kernel event queue.
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        return wait_child_poll_sigfd(child, deadline, sig_fd);
    }
    let _ = set_cloexec(kq);

    let pid = child.id() as usize;
    let changes = [
        libc::kevent {
            ident: pid,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        },
        libc::kevent {
            ident: sig_fd as usize,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        },
    ];

    // SAFETY: Registering `EVFILT_PROC` and `EVFILT_READ` on `kq`.
    let reg_rc = unsafe {
        libc::kevent(
            kq,
            changes.as_ptr(),
            changes.len() as libc::c_int,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
        )
    };
    if reg_rc < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(kq) };
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(WaitOutcome::Exited(child.wait()?));
        }
        return wait_child_poll_sigfd(child, deadline, sig_fd);
    }

    let outcome = loop {
        if let Some(st) = child.try_wait()? {
            break Ok(WaitOutcome::Exited(st));
        }
        let ts = match deadline {
            None => None,
            Some(dl) => {
                let rem = dl.saturating_duration_since(Instant::now());
                if rem.is_zero() {
                    break Ok(WaitOutcome::TimedOut);
                }
                Some(libc::timespec {
                    tv_sec: rem.as_secs() as libc::time_t,
                    tv_nsec: rem.subsec_nanos() as libc::c_long,
                })
            }
        };
        let ts_ptr = ts.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
        let mut events: [libc::kevent; 2] = unsafe { std::mem::zeroed() };
        // SAFETY: Valid writable event array and optional timespec pointer.
        let n = unsafe {
            libc::kevent(
                kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                events.len() as libc::c_int,
                ts_ptr,
            )
        };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                if let Some(st) = child.try_wait()? {
                    break Ok(WaitOutcome::Exited(st));
                }
                break Ok(WaitOutcome::SignalWakeup);
            }
            break Err(err);
        }
        if n == 0 {
            if let Some(st) = child.try_wait()? {
                break Ok(WaitOutcome::Exited(st));
            }
            break Ok(WaitOutcome::TimedOut);
        }
        if let Some(st) = child.try_wait()? {
            break Ok(WaitOutcome::Exited(st));
        }
        if events[..n as usize]
            .iter()
            .any(|e| e.filter == libc::EVFILT_READ)
        {
            break Ok(WaitOutcome::SignalWakeup);
        }
    };

    // SAFETY: Close owned kqueue FD unconditionally.
    unsafe { libc::close(kq) };
    outcome
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: RawFd,
) -> io::Result<WaitOutcome> {
    wait_child_poll_sigfd(child, deadline, sig_fd)
}

fn wait_child_poll_sigfd(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: RawFd,
) -> io::Result<WaitOutcome> {
    if let Some(st) = child.try_wait()? {
        return Ok(WaitOutcome::Exited(st));
    }
    let timeout_ms = deadline_to_poll_ms(deadline);
    if timeout_ms == 0 {
        return Ok(WaitOutcome::TimedOut);
    }
    let mut pfd = libc::pollfd {
        fd: sig_fd,
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: Valid single-element `pollfd`.
    let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
    if let Some(st) = child.try_wait()? {
        return Ok(WaitOutcome::Exited(st));
    }
    if rc == 0 {
        return Ok(WaitOutcome::TimedOut);
    }
    Ok(WaitOutcome::SignalWakeup)
}

fn terminate_process_group(
    pgid: libc::pid_t,
    child: &mut Child,
    sig_fd: RawFd,
) -> Result<ExitStatus, ExecError> {
    if pgid > 0 {
        // SAFETY: Sending SIGTERM to the isolated child process group `-pgid`.
        unsafe {
            libc::kill(-pgid, libc::SIGTERM);
        }
    }
    let grace_end = Instant::now() + KILL_GRACE_PERIOD;
    let mut reaped_status: Option<ExitStatus> = None;

    loop {
        if reaped_status.is_none()
            && let Some(st) = child.try_wait().map_err(ExecError::Wait)?
        {
            reaped_status = Some(st);
        }
        if let Some(st) = reaped_status {
            // SAFETY: Signal 0 only probes whether any member remains in `-pgid`.
            if pgid <= 0 || unsafe { libc::kill(-pgid, 0) } == -1 {
                return Ok(st);
            }
            let rem = grace_end.saturating_duration_since(Instant::now());
            if rem.is_zero() {
                break;
            }
            // Wait briefly for remaining grandchildren in the same process group without busy spinning.
            let wait_ms = rem.as_millis().clamp(1, 50) as libc::c_int;
            let mut pfd = libc::pollfd {
                fd: sig_fd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: Valid single-element `pollfd` waiting for `SIGCHLD` or short slice.
            unsafe {
                let _ = libc::poll(&mut pfd, 1, wait_ms);
            }
            continue;
        }
        if Instant::now() >= grace_end {
            break;
        }
        match wait_child_until(child, Some(grace_end), sig_fd).map_err(ExecError::Wait)? {
            WaitOutcome::Exited(st) => reaped_status = Some(st),
            WaitOutcome::SignalWakeup => continue,
            WaitOutcome::TimedOut => break,
        }
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

fn create_nonblock_pipe_cloexec() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` is a valid 2-element array for `pipe(2)`.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    for &fd in &fds {
        if let Err(err) = set_cloexec(fd).and_then(|()| set_nonblock(fd)) {
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            return Err(err);
        }
    }
    Ok((fds[0], fds[1]))
}

fn set_cloexec(fd: RawFd) -> io::Result<()> {
    // SAFETY: `F_SETFD(FD_CLOEXEC)` on valid FD.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn set_nonblock(fd: RawFd) -> io::Result<()> {
    // SAFETY: Query and set `O_NONBLOCK` on valid FD.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
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
