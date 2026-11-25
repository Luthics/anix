//! macOS / BSD pure event-driven primitives (`kqueue` `EVFILT_PROC` / `EVFILT_VNODE`)
//! and POSIX platform capabilities (v1.1 §§4.2, 6.3 & 10.2).

use std::ffi::CString;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

use super::{ChildWaitOutcome, PlatformCapabilities, SandboxPolicy, posix};

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

/// Event-driven child wait using macOS/BSD `kqueue(2)` (`EVFILT_PROC` / `NOTE_EXIT`)
/// multiplexed with optional `sig_fd` (`EVFILT_READ`) and exact timeout (v1.1 §6.3).
pub fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: Option<RawFd>,
) -> io::Result<ChildWaitOutcome> {
    if let Some(st) = child.try_wait()? {
        return Ok(ChildWaitOutcome::Exited(st));
    }

    // SAFETY: `kqueue(2)` creates an isolated kernel event queue.
    let kq = unsafe { libc::kqueue() };
    if kq < 0 {
        return Err(io::Error::last_os_error());
    }
    let _ = posix::set_cloexec(kq);

    let pid = child.id() as libc::uintptr_t;
    let mut changes = Vec::with_capacity(2);
    changes.push(libc::kevent {
        ident: pid,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    });
    if let Some(sfd) = sig_fd {
        changes.push(libc::kevent {
            ident: sfd as libc::uintptr_t,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD | libc::EV_ENABLE,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        });
    }

    // Register changes non-blockingly first so we immediately catch `ESRCH` (already exited).
    let zero_ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut reg_events = [libc::kevent {
        ident: 0,
        filter: 0,
        flags: 0,
        fflags: 0,
        data: 0,
        udata: std::ptr::null_mut(),
    }; 2];
    // SAFETY: `changes` and `reg_events` are valid initialized `kevent` slices.
    let reg_rc = unsafe {
        libc::kevent(
            kq,
            changes.as_ptr(),
            changes.len() as libc::c_int,
            reg_events.as_mut_ptr(),
            reg_events.len() as libc::c_int,
            &zero_ts,
        )
    };
    if reg_rc < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(kq) };
        if err.raw_os_error() == Some(libc::ESRCH) {
            return Ok(ChildWaitOutcome::Exited(child.wait()?));
        }
        return Err(err);
    }

    let mut early_sig = false;
    for ev in &reg_events[..reg_rc as usize] {
        if ev.flags & libc::EV_ERROR != 0 && ev.data == libc::ESRCH as isize {
            unsafe { libc::close(kq) };
            return Ok(ChildWaitOutcome::Exited(child.wait()?));
        }
        if ev.filter == libc::EVFILT_PROC {
            unsafe { libc::close(kq) };
            return Ok(ChildWaitOutcome::Exited(child.wait()?));
        }
        if ev.filter == libc::EVFILT_READ {
            early_sig = true;
        }
    }
    if early_sig {
        if let Some(st) = child.try_wait()? {
            unsafe { libc::close(kq) };
            return Ok(ChildWaitOutcome::Exited(st));
        }
        unsafe { libc::close(kq) };
        return Ok(ChildWaitOutcome::SignalWakeup);
    }

    let outcome = loop {
        if let Some(st) = child.try_wait()? {
            break Ok(ChildWaitOutcome::Exited(st));
        }

        let ts_storage;
        let ts_ptr = match deadline {
            None => std::ptr::null(),
            Some(dl) => {
                let rem = dl.saturating_duration_since(Instant::now());
                if rem.is_zero() {
                    break Ok(ChildWaitOutcome::TimedOut);
                }
                ts_storage = libc::timespec {
                    tv_sec: rem.as_secs() as libc::time_t,
                    tv_nsec: rem.subsec_nanos() as libc::c_long,
                };
                &ts_storage as *const libc::timespec
            }
        };

        let mut events = [libc::kevent {
            ident: 0,
            filter: 0,
            flags: 0,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        }; 2];
        // SAFETY: `events` holds 2 writable `kevent`s; `ts_ptr` is null (infinite) or valid timespec.
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
        if events[..n as usize]
            .iter()
            .any(|e| e.filter == libc::EVFILT_READ)
        {
            break Ok(ChildWaitOutcome::SignalWakeup);
        }
    };

    // SAFETY: `kq` is exclusively owned by this wait call.
    unsafe { libc::close(kq) };
    outcome
}

/// macOS `kqueue` (`EVFILT_VNODE` + `EVFILT_READ` signal pipe) directory watcher for
/// Maildir `--inbox <DIR>/new/` with exact timeout (v1.1 §§4.2 & 6.3).
#[derive(Debug)]
pub struct DirWatcher {
    kq: RawFd,
    dir_fd: RawFd,
}

impl DirWatcher {
    pub fn watch(dir: &Path, sig_fd: Option<RawFd>) -> io::Result<Self> {
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

        let mut changes = vec![libc::kevent {
            ident: dir_fd as libc::uintptr_t,
            filter: libc::EVFILT_VNODE,
            flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR,
            fflags: libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_RENAME | libc::NOTE_ATTRIB,
            data: 0,
            udata: std::ptr::null_mut(),
        }];
        if let Some(sfd) = sig_fd {
            changes.push(libc::kevent {
                ident: sfd as libc::uintptr_t,
                filter: libc::EVFILT_READ,
                flags: libc::EV_ADD | libc::EV_ENABLE,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            });
        }
        // SAFETY: `kq` is valid and `changes` points to initialized `kevent`s.
        let rc = unsafe {
            libc::kevent(
                kq,
                changes.as_ptr(),
                changes.len() as libc::c_int,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
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

    /// Block with exact `timeout` (`None` = infinite block, 0 wakeups/sec) waiting for
    /// directory writes/renames or signal wakeup.
    pub fn wait(&self, timeout: Option<Duration>) -> io::Result<bool> {
        let mut events = [libc::kevent {
            ident: 0,
            filter: 0,
            flags: 0,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        }; 2];
        let ts_storage;
        let ts_ptr = match timeout {
            None => std::ptr::null(),
            Some(d) => {
                ts_storage = libc::timespec {
                    tv_sec: d.as_secs() as libc::time_t,
                    tv_nsec: d.subsec_nanos() as libc::c_long,
                };
                &ts_storage as *const libc::timespec
            }
        };
        // SAFETY: `events` holds 2 writable `kevent`s; `ts_ptr` is null (infinite) or valid timespec.
        let n = unsafe {
            libc::kevent(
                self.kq,
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
                return Ok(false);
            }
            return Err(err);
        }
        Ok(events[..n as usize]
            .iter()
            .any(|e| e.filter == libc::EVFILT_VNODE))
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
