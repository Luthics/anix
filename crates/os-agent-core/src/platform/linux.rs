//! Linux ≥ 5.13 hardened primitives: `pidfd_open`, `PR_SET_PDEATHSIG`, Landlock LSM,
//! and `inotify`-backed Maildir `--inbox` watcher (Sections 4.2 & 10.2).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};

use super::{PlatformCapabilities, SandboxPolicy, posix};

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

/// Linux `inotify`-backed directory watcher for Maildir `--inbox <DIR>/new/` (Section 4.2).
#[derive(Debug)]
pub struct DirWatcher {
    ifd: RawFd,
}

impl DirWatcher {
    pub fn watch(dir: &Path) -> io::Result<Self> {
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

        Ok(Self { ifd })
    }

    /// Block up to `timeout_ms` waiting for directory activity; drains pending events.
    pub fn wait(&self, timeout_ms: i32) -> io::Result<bool> {
        let mut pfd = libc::pollfd {
            fd: self.ifd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: `pfd` points to 1 initialized `pollfd`.
        let rc = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
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

        // Drain all queued inotify events so the fd returns to idle.
        let mut buf = [0u8; 4096];
        loop {
            // SAFETY: Non-blocking read from owned `ifd` into `buf`.
            let n =
                unsafe { libc::read(self.ifd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
        }
        Ok(true)
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        // SAFETY: `ifd` is exclusively owned by `DirWatcher`.
        unsafe {
            libc::close(self.ifd);
        }
    }
}
