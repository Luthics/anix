//! macOS / BSD fallback stubs (pure POSIX mode) and `kqueue` `EVFILT_VNODE` watcher
//! for Maildir `--inbox <DIR>/new/` (Sections 4.2 & 10.2).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;

use super::{PlatformCapabilities, SandboxPolicy, posix};

pub fn probe_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        dev_fd_available: posix::dev_fd_available(),
        pidfd_supported: false,
        landlock_abi: None,
    }
}

#[derive(Debug, Clone)]
pub struct PreparedSandbox;

pub fn prepare_sandbox(
    policy: SandboxPolicy,
    _cwd: &Path,
    _runtime_dir: &Path,
) -> Result<Option<PreparedSandbox>, String> {
    match policy {
        SandboxPolicy::Off => Ok(None),
        SandboxPolicy::Strict => {
            Err("Landlock LSM sandbox is only supported on Linux >= 5.13 (--sandbox=strict)".into())
        }
        SandboxPolicy::BestEffort => {
            eprintln!(
                "agent: warning: Landlock LSM is unavailable on this platform; degrading to pure POSIX mode"
            );
            Ok(None)
        }
    }
}

/// # Safety
/// No-op on non-Linux platforms.
pub unsafe fn apply_prepared_sandbox_in_child(_sb: &PreparedSandbox) -> io::Result<()> {
    Ok(())
}

/// macOS `kqueue` (`EVFILT_VNODE`) directory watcher for Maildir `--inbox <DIR>/new/` (Section 4.2).
#[derive(Debug)]
pub struct DirWatcher {
    kq: RawFd,
    dir_fd: RawFd,
}

impl DirWatcher {
    pub fn watch(dir: &Path) -> io::Result<Self> {
        let cpath = CString::new(dir.as_os_str().as_bytes())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

        // SAFETY: `O_EVTONLY | O_CLOEXEC` opens a directory descriptor purely for kqueue monitoring.
        let dir_fd = unsafe { libc::open(cpath.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        if dir_fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: `kqueue(2)` creates an isolated kernel event queue.
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(dir_fd) };
            return Err(err);
        }
        let _ = posix::set_cloexec(kq);

        let change = libc::kevent {
            ident: dir_fd as libc::uintptr_t,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_RENAME | libc::NOTE_ATTRIB,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        // SAFETY: `kq` is valid and `change` points to 1 initialized `kevent`.
        let rc = unsafe { libc::kevent(kq, &change, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
        if rc < 0 {
            let err = io::Error::last_os_error();
            unsafe {
                libc::close(kq);
                libc::close(dir_fd);
            }
            return Err(err);
        }

        Ok(Self { kq, dir_fd })
    }

    /// Block up to `timeout_ms` waiting for directory writes/renames.
    pub fn wait(&self, timeout_ms: i32) -> io::Result<bool> {
        let mut ev = libc::kevent {
            ident: 0,
            filter: 0,
            flags: 0,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        let ts = libc::timespec {
            tv_sec: (timeout_ms.max(0) / 1000) as libc::time_t,
            tv_nsec: ((timeout_ms.max(0) % 1000) as libc::c_long) * 1_000_000,
        };
        // SAFETY: `ev` holds 1 writable `kevent` and `ts` is a valid timeout.
        let n = unsafe { libc::kevent(self.kq, std::ptr::null(), 0, &mut ev, 1, &ts) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(false);
            }
            return Err(err);
        }
        Ok(n > 0)
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        // SAFETY: Both `kq` and `dir_fd` are exclusively owned by `DirWatcher`.
        unsafe {
            libc::close(self.kq);
            libc::close(self.dir_fd);
        }
    }
}
