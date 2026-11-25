//! Linux ≥ 5.13 hardened primitives: `pidfd_open`, `PR_SET_PDEATHSIG`, Landlock LSM,
//! and `inotify`-backed Maildir `--inbox` watcher (Sections 4.2 & 10.2).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use super::{ChildWaitOutcome, PlatformCapabilities, SandboxPolicy, posix};

const LANDLOCK_CREATE_RULESET_VERSION: u32 = 1 << 0;
const LANDLOCK_RULE_PATH_BENEATH: u32 = 1;

// Landlock ABI v1 filesystem access bits (supported universally on Linux >= 5.13).
const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
// ABI v2+
const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
// ABI v3+
const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;

const FS_ALL_V1: u64 = LANDLOCK_ACCESS_FS_EXECUTE
    | LANDLOCK_ACCESS_FS_WRITE_FILE
    | LANDLOCK_ACCESS_FS_READ_FILE
    | LANDLOCK_ACCESS_FS_READ_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_DIR
    | LANDLOCK_ACCESS_FS_REMOVE_FILE
    | LANDLOCK_ACCESS_FS_MAKE_CHAR
    | LANDLOCK_ACCESS_FS_MAKE_DIR
    | LANDLOCK_ACCESS_FS_MAKE_REG
    | LANDLOCK_ACCESS_FS_MAKE_SOCK
    | LANDLOCK_ACCESS_FS_MAKE_FIFO
    | LANDLOCK_ACCESS_FS_MAKE_BLOCK
    | LANDLOCK_ACCESS_FS_MAKE_SYM;

const FS_READ_ONLY: u64 =
    LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;

#[repr(C)]
struct LandlockRulesetAttr {
    handled_access_fs: u64,
}

#[repr(C, packed)]
struct LandlockPathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Probe the kernel's Landlock ABI version (`None` if disabled/unsupported).
pub fn landlock_abi_version() -> Option<u32> {
    // SAFETY: Passing NULL attr with size 0 and LANDLOCK_CREATE_RULESET_VERSION is a read-only
    // kernel query defined by Linux's `landlock_create_ruleset(2)`.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<libc::c_void>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if ret >= 1 { Some(ret as u32) } else { None }
}

/// Probe whether `pidfd_open(2)` is supported by the running kernel.
pub fn pidfd_supported() -> bool {
    // SAFETY: Opening and immediately closing a pidfd for our own PID has no side effects.
    let pid = unsafe { libc::getpid() };
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };
    if fd >= 0 {
        unsafe {
            libc::close(fd as libc::c_int);
        }
        true
    } else {
        false
    }
}

pub fn probe_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        dev_fd_available: posix::dev_fd_available(),
        pidfd_supported: pidfd_supported(),
        landlock_abi: landlock_abi_version(),
    }
}

/// Pre-computed Landlock policy paths (resolved in parent BEFORE `fork` so `pre_exec`
/// only invokes async-signal-safe syscalls!).
#[derive(Debug, Clone)]
pub struct PreparedSandbox {
    pub abi: u32,
    pub rw_dirs: Vec<CString>,
    pub rw_files: Vec<CString>,
    pub root: CString,
}

/// Prepare Landlock sandbox rules in the parent process prior to spawning `msh`.
pub fn prepare_sandbox(
    policy: SandboxPolicy,
    cwd: &Path,
    runtime_dir: &Path,
) -> Result<Option<PreparedSandbox>, String> {
    if policy == SandboxPolicy::Off {
        return Ok(None);
    }

    let Some(abi) = landlock_abi_version() else {
        return if policy == SandboxPolicy::Strict {
            Err(
                "Landlock LSM is not supported or enabled on this Linux kernel (--sandbox=strict)"
                    .into(),
            )
        } else {
            eprintln!(
                "agent: warning: Landlock LSM unavailable on this kernel; running without filesystem sandbox"
            );
            Ok(None)
        };
    };

    let mut rw_dirs_paths: Vec<PathBuf> = vec![cwd.to_path_buf(), PathBuf::from("/tmp")];
    if runtime_dir.exists() {
        rw_dirs_paths.push(runtime_dir.to_path_buf());
    }

    let mut rw_dirs = Vec::new();
    for p in rw_dirs_paths {
        let resolved = std::fs::canonicalize(&p).unwrap_or(p);
        if resolved.exists()
            && let Ok(cs) = CString::new(resolved.as_os_str().as_bytes())
        {
            rw_dirs.push(cs);
        }
    }

    let mut rw_files = Vec::new();
    for dev in ["/dev/null", "/dev/zero", "/dev/full", "/dev/tty"] {
        if Path::new(dev).exists()
            && let Ok(cs) = CString::new(dev)
        {
            rw_files.push(cs);
        }
    }

    Ok(Some(PreparedSandbox {
        abi,
        rw_dirs,
        rw_files,
        root: CString::new("/").expect("static root cstring"),
    }))
}

/// Apply a prepared Landlock ruleset inside `pre_exec` (async-signal-safe syscalls only).
///
/// # Safety
/// Must be called after `fork()` in the child process before `execve()`.
pub unsafe fn apply_prepared_sandbox_in_child(sb: &PreparedSandbox) -> io::Result<()> {
    let mut handled = FS_ALL_V1;
    if sb.abi >= 2 {
        handled |= LANDLOCK_ACCESS_FS_REFER;
    }
    if sb.abi >= 3 {
        handled |= LANDLOCK_ACCESS_FS_TRUNCATE;
    }

    let attr = LandlockRulesetAttr {
        handled_access_fs: handled,
    };

    // SAFETY: `attr` is valid for `size_of::<LandlockRulesetAttr>()` bytes.
    let ruleset_fd = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &attr as *const LandlockRulesetAttr,
            std::mem::size_of::<LandlockRulesetAttr>(),
            0u32,
        )
    };
    if ruleset_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let ruleset_fd = ruleset_fd as libc::c_int;

    let add_path_rule = |path: &CString, allowed: u64| -> io::Result<()> {
        // SAFETY: `path` is NUL-terminated; `O_PATH | O_CLOEXEC` opens a path handle safely.
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if fd < 0 {
            return Ok(());
        }
        let rule = LandlockPathBeneathAttr {
            allowed_access: allowed,
            parent_fd: fd,
        };
        // SAFETY: `ruleset_fd` and `rule` are valid Linux Landlock descriptors/structs.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_landlock_add_rule,
                ruleset_fd,
                LANDLOCK_RULE_PATH_BENEATH,
                &rule as *const LandlockPathBeneathAttr,
                0u32,
            )
        };
        // SAFETY: Closing the temporary O_PATH handle.
        unsafe { libc::close(fd) };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    };

    let res = (|| {
        add_path_rule(&sb.root, FS_READ_ONLY)?;
        for dir in &sb.rw_dirs {
            add_path_rule(dir, handled)?;
        }
        let dev_file_access = LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_WRITE_FILE;
        for dev in &sb.rw_files {
            add_path_rule(dev, dev_file_access)?;
        }

        // SAFETY: `PR_SET_NO_NEW_PRIVS` is required before `landlock_restrict_self(2)`.
        if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Enforce the constructed Landlock ruleset on the current child process.
        if unsafe { libc::syscall(libc::SYS_landlock_restrict_self, ruleset_fd, 0u32) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })();

    // SAFETY: Close the ruleset fd unconditionally.
    unsafe { libc::close(ruleset_fd) };
    res
}

/// Bind child lifetime to parent using `PR_SET_PDEATHSIG(SIGKILL)` with parent-PID race check.
///
/// # Safety
/// Intended to be called in `CommandExt::pre_exec` immediately after `fork()`.
pub unsafe fn set_pdeathsig_sigkill(parent_pid: libc::pid_t) -> io::Result<()> {
    // SAFETY: `prctl(PR_SET_PDEATHSIG, SIGKILL)` is async-signal-safe.
    if unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Guard against parent having exited before `prctl` took effect.
    if unsafe { libc::getppid() } != parent_pid {
        unsafe { libc::_exit(1) };
    }
    Ok(())
}

/// Event-driven child wait using Linux `pidfd_open(2)` + `epoll(7)` (v1.1 §6.3).
/// Falls back cleanly to `sig_fd` (`SIGCHLD` wakeup) if `pidfd_open` is unavailable on old kernels.
pub fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: Option<RawFd>,
) -> io::Result<ChildWaitOutcome> {
    if let Some(st) = child.try_wait()? {
        return Ok(ChildWaitOutcome::Exited(st));
    }

    let pid = child.id() as libc::pid_t;
    // SAFETY: `pidfd_open(pid, 0)` returns a pollable FD that becomes readable upon child exit.
    let pidfd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0u32) };
    if pidfd < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(ChildWaitOutcome::Exited(child.wait()?));
        }
        // Fallback when `pidfd_open` is unsupported (`ENOSYS`/`EPERM`): wait on `sig_fd` (which
        // receives `SIGCHLD` + termination signals) with exact remaining timeout.
        if let Some(sfd) = sig_fd {
            if let Some(st) = child.try_wait()? {
                return Ok(ChildWaitOutcome::Exited(st));
            }
            let timeout_ms = posix::deadline_to_poll_ms(deadline);
            if timeout_ms == 0 {
                return Ok(ChildWaitOutcome::TimedOut);
            }
            let mut pfd = libc::pollfd {
                fd: sfd,
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: Valid single-element pollfd slice.
            let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if let Some(st) = child.try_wait()? {
                return Ok(ChildWaitOutcome::Exited(st));
            }
            if rc == 0 {
                return Ok(ChildWaitOutcome::TimedOut);
            }
            return Ok(ChildWaitOutcome::SignalWakeup);
        }
        return Ok(ChildWaitOutcome::Exited(child.wait()?));
    }
    let pidfd = pidfd as RawFd;
    let _ = posix::set_cloexec(pidfd);

    // SAFETY: `epoll_create1(EPOLL_CLOEXEC)` creates an isolated kernel event instance.
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
    // SAFETY: Registering `pidfd` on `epfd`.
    let add_pid = unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, pidfd, &mut ev_pid) };
    if add_pid < 0 {
        let err = io::Error::last_os_error();
        unsafe {
            libc::close(epfd);
            libc::close(pidfd);
        }
        return Err(err);
    }

    if let Some(sfd) = sig_fd {
        let mut ev_sig = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: 2,
        };
        // SAFETY: Registering `sig_fd` on `epfd`.
        let _ = unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, sfd, &mut ev_sig) };
    }

    let outcome = loop {
        // Re-check after registration to close any exit race between `try_wait` and `pidfd_open`.
        if let Some(st) = child.try_wait()? {
            break Ok(ChildWaitOutcome::Exited(st));
        }
        let timeout_ms = posix::deadline_to_poll_ms(deadline);
        if timeout_ms == 0 {
            break Ok(ChildWaitOutcome::TimedOut);
        }
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 2];
        // SAFETY: `events` is a valid writable array of 2 `epoll_event`s.
        let n = unsafe { libc::epoll_wait(epfd, events.as_mut_ptr(), 2, timeout_ms) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                if let Some(st) = child.try_wait()? {
                    break Ok(ChildWaitOutcome::Exited(st));
                }
                break Ok(ChildWaitOutcome::SignalWakeup);
            }
            break Err(err);
        }
        if n == 0 {
            if let Some(st) = child.try_wait()? {
                break Ok(ChildWaitOutcome::Exited(st));
            }
            break Ok(ChildWaitOutcome::TimedOut);
        }
        if let Some(st) = child.try_wait()? {
            break Ok(ChildWaitOutcome::Exited(st));
        }
        let saw_sig = events[..n as usize].iter().any(|e| e.u64 == 2);
        if saw_sig {
            break Ok(ChildWaitOutcome::SignalWakeup);
        }
    };

    // SAFETY: Close both kernel descriptors unconditionally.
    unsafe {
        libc::close(epfd);
        libc::close(pidfd);
    }
    outcome
}

/// Linux `inotify` + `epoll` directory watcher for Maildir `--inbox <DIR>/new/` (v1.1 §§4.2 & 6.3).
#[derive(Debug)]
pub struct DirWatcher {
    epfd: RawFd,
    ifd: RawFd,
}

impl DirWatcher {
    pub fn watch(dir: &Path, sig_fd: Option<RawFd>) -> io::Result<Self> {
        let cpath = CString::new(dir.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        // SAFETY: `inotify_init1(IN_NONBLOCK | IN_CLOEXEC)` creates an isolated inotify handle.
        let ifd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if ifd < 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: `ifd` is valid and `cpath` is NUL-terminated.
        let wd = unsafe {
            libc::inotify_add_watch(
                ifd,
                cpath.as_ptr(),
                libc::IN_MOVED_TO | libc::IN_CREATE | libc::IN_CLOSE_WRITE,
            )
        };
        if wd < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(ifd) };
            return Err(err);
        }

        // SAFETY: `epoll_create1(EPOLL_CLOEXEC)` multiplexes `inotify` and `sig_fd` with zero polling.
        let epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epfd < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(ifd) };
            return Err(err);
        }

        let mut ev_in = libc::epoll_event {
            events: libc::EPOLLIN as u32,
            u64: 1,
        };
        if unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, ifd, &mut ev_in) } < 0 {
            let err = io::Error::last_os_error();
            unsafe {
                libc::close(epfd);
                libc::close(ifd);
            }
            return Err(err);
        }

        if let Some(sfd) = sig_fd {
            let mut ev_sig = libc::epoll_event {
                events: libc::EPOLLIN as u32,
                u64: 2,
            };
            let _ = unsafe { libc::epoll_ctl(epfd, libc::EPOLL_CTL_ADD, sfd, &mut ev_sig) };
        }

        Ok(Self { epfd, ifd })
    }

    /// Block with exact `timeout` (`None` = infinite block, 0 wakeups/sec) waiting for directory
    /// activity or signal wakeup; drains pending inotify events.
    pub fn wait(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let timeout_ms = match timeout {
            None => -1,
            Some(d) => {
                let ms = d.as_millis();
                if d.is_zero() {
                    0
                } else if ms == 0 {
                    1
                } else {
                    ms.min(libc::c_int::MAX as u128) as libc::c_int
                }
            }
        };
        let mut events = [libc::epoll_event { events: 0, u64: 0 }; 2];
        // SAFETY: `events` holds 2 writable `epoll_event`s.
        let rc = unsafe { libc::epoll_wait(self.epfd, events.as_mut_ptr(), 2, timeout_ms) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(false);
            }
            return Err(err);
        }
        if rc == 0 {
            return Ok(false);
        }

        let mut dir_ready = false;
        for ev in &events[..rc as usize] {
            if ev.u64 == 1 {
                dir_ready = true;
            }
        }
        if dir_ready {
            let mut buf = [0u8; 4096];
            loop {
                // SAFETY: Non-blocking read from owned `ifd` into `buf`.
                let n = unsafe {
                    libc::read(self.ifd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
                };
                if n <= 0 {
                    break;
                }
            }
        }
        Ok(dir_ready)
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        // SAFETY: Both `epfd` and `ifd` are exclusively owned by `DirWatcher`.
        unsafe {
            libc::close(self.epfd);
            libc::close(self.ifd);
        }
    }
}
