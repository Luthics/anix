//! POSIX signal handling with non-blocking self-pipe wakeup for pure event-driven waits:
//! `SIGINT` (130), `SIGTERM` (143), `SIGUSR1` (compact), `SIGUSR2` (IR JSONL snapshot) (v1.1 §§6.3 & 6.4).

use std::os::unix::io::RawFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::IrStream;

use crate::platform::posix;

const CHILD_KILL_GRACE: Duration = Duration::from_secs(3);

/// Owned self-pipe pair closed when the last [`SignalState`] clone drops.
#[derive(Debug)]
struct WakeupPipe {
    read_fd: RawFd,
    write_fd: RawFd,
}

impl Drop for WakeupPipe {
    fn drop(&mut self) {
        // SAFETY: Both ends are exclusively owned by `WakeupPipe`.
        unsafe {
            libc::close(self.read_fd);
            libc::close(self.write_fd);
        }
    }
}

/// Shared atomic signal flags, event-loop self-pipe wakeup FD, and active child PGID registry.
#[derive(Debug, Clone)]
pub struct SignalState {
    pub sigint: Arc<AtomicBool>,
    pub sigterm: Arc<AtomicBool>,
    pub sigusr1: Arc<AtomicBool>,
    pub sigusr2: Arc<AtomicBool>,
    active_child_pgid: Arc<AtomicI32>,
    wakeup: Arc<WakeupPipe>,
}

impl SignalState {
    /// Register async-signal-safe handlers for `SIGINT`, `SIGTERM`, `SIGUSR1`, `SIGUSR2`, `SIGCHLD`
    /// with self-pipe wakeup and ignore `SIGPIPE`.
    pub fn install() -> std::io::Result<Self> {
        posix::ignore_sigpipe();

        let (read_fd, write_fd) = posix::create_pipe_cloexec()?;
        posix::set_nonblocking(read_fd)?;
        posix::set_nonblocking(write_fd)?;

        let state = Self {
            sigint: Arc::new(AtomicBool::new(false)),
            sigterm: Arc::new(AtomicBool::new(false)),
            sigusr1: Arc::new(AtomicBool::new(false)),
            sigusr2: Arc::new(AtomicBool::new(false)),
            active_child_pgid: Arc::new(AtomicI32::new(0)),
            wakeup: Arc::new(WakeupPipe { read_fd, write_fd }),
        };

        // Double Ctrl+C guard (registered BEFORE flag+wakeup so 1st SIGINT triggers graceful
        // child reaping and 2nd SIGINT immediately SIGKILLs the active child PGID and exits 130).
        let sigint_force = Arc::clone(&state.sigint);
        let pgid_force = Arc::clone(&state.active_child_pgid);
        // SAFETY: Handler invokes only POSIX async-signal-safe operations (`Atomic::load`, `kill`, `_exit`).
        unsafe {
            signal_hook::low_level::register(signal_hook::consts::SIGINT, move || {
                if sigint_force.load(Ordering::SeqCst) {
                    let pgid = pgid_force.load(Ordering::SeqCst);
                    if pgid > 0 {
                        libc::kill(-pgid, libc::SIGKILL);
                    }
                    libc::_exit(ExitCode::SigInt.as_i32());
                }
            })?;
        }

        register_flag_and_wake(
            signal_hook::consts::SIGINT,
            Arc::clone(&state.sigint),
            write_fd,
        )?;
        register_flag_and_wake(
            signal_hook::consts::SIGTERM,
            Arc::clone(&state.sigterm),
            write_fd,
        )?;
        register_flag_and_wake(
            signal_hook::consts::SIGUSR1,
            Arc::clone(&state.sigusr1),
            write_fd,
        )?;
        register_flag_and_wake(
            signal_hook::consts::SIGUSR2,
            Arc::clone(&state.sigusr2),
            write_fd,
        )?;
        // Also wake on `SIGCHLD` as a zero-poll fallback if `pidfd_open` is unavailable on old Linux.
        // SAFETY: Handler performs only a non-blocking single-byte `write(2)` (POSIX async-signal-safe).
        unsafe {
            signal_hook::low_level::register(signal_hook::consts::SIGCHLD, move || {
                let b: u8 = b'c';
                let _ = libc::write(write_fd, &b as *const u8 as *const libc::c_void, 1);
            })?;
        }

        Ok(state)
    }

    /// Readable non-blocking self-pipe descriptor signaled on any handled POSIX signal.
    pub fn wakeup_fd(&self) -> RawFd {
        self.wakeup.read_fd
    }

    /// Drain pending wakeup bytes from the non-blocking self-pipe so it returns to idle.
    pub fn drain_wakeup(&self) {
        let mut buf = [0u8; 64];
        loop {
            // SAFETY: Non-blocking read from owned self-pipe read end.
            let n = unsafe {
                libc::read(
                    self.wakeup.read_fd,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    buf.len(),
                )
            };
            if n <= 0 {
                break;
            }
        }
    }

    pub fn set_active_child_pgid(&self, pgid: libc::pid_t) {
        self.active_child_pgid.store(pgid, Ordering::SeqCst);
    }

    pub fn clear_active_child_pgid(&self) {
        self.active_child_pgid.store(0, Ordering::SeqCst);
    }

    /// Check whether `SIGINT` or `SIGTERM` has been received.
    pub fn termination_exit_code(&self) -> Option<ExitCode> {
        if self.sigint.load(Ordering::SeqCst) {
            Some(ExitCode::SigInt)
        } else if self.sigterm.load(Ordering::SeqCst) {
            Some(ExitCode::SigTerm)
        } else {
            None
        }
    }

    /// Atomically consume a pending `SIGUSR1` compact request.
    pub fn take_compact_request(&self) -> bool {
        self.sigusr1.swap(false, Ordering::SeqCst)
    }

    /// Atomically consume a pending `SIGUSR2` snapshot dump request.
    pub fn take_snapshot_request(&self) -> bool {
        self.sigusr2.swap(false, Ordering::SeqCst)
    }

    /// Gracefully terminate the currently active child process group (Provider / `msh`):
    /// `SIGTERM` → wait up to 3s → `SIGKILL`.
    pub fn terminate_active_child(&self) {
        let pgid = self.active_child_pgid.swap(0, Ordering::SeqCst);
        if pgid > 0 {
            posix::terminate_process_group(pgid, CHILD_KILL_GRACE);
        }
    }

    /// Gracefully terminate and reap `child`'s process group (`SIGTERM` → event-driven wait up to 3s → `SIGKILL`).
    pub fn terminate_active_child_and_wait(
        &self,
        child: &mut std::process::Child,
    ) -> std::io::Result<std::process::ExitStatus> {
        let pgid = self.active_child_pgid.swap(0, Ordering::SeqCst);
        let effective_pgid = if pgid > 0 {
            pgid
        } else {
            child.id() as libc::pid_t
        };
        posix::terminate_child_process_group(child, effective_pgid, CHILD_KILL_GRACE)
    }
}

fn register_flag_and_wake(
    sig: libc::c_int,
    flag: Arc<AtomicBool>,
    write_fd: RawFd,
) -> std::io::Result<()> {
    // SAFETY: Handler invokes only `AtomicBool::store` and non-blocking `write(2)` (async-signal-safe).
    unsafe {
        signal_hook::low_level::register(sig, move || {
            flag.store(true, Ordering::SeqCst);
            let b: u8 = b's';
            let _ = libc::write(write_fd, &b as *const u8 as *const libc::c_void, 1);
        })?;
    }
    Ok(())
}

/// Atomically dump the current in-memory Canonical IR JSONL snapshot on `SIGUSR2`
/// to `/tmp/anix_<pid>_<epoch_ms>.ctx.jsonl` (mode `0600`) and report the path to `fd 2`.
pub fn dump_ir_snapshot(stream: &IrStream) -> std::io::Result<PathBuf> {
    let jsonl = stream
        .to_jsonl()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = PathBuf::from(format!(
        "/tmp/anix_{}_{}.ctx.jsonl",
        std::process::id(),
        posix::timestamp_epoch_ms()
    ));
    posix::write_atomic_0600(&path, jsonl.as_bytes())?;
    eprintln!("{}", path.display());
    Ok(path)
}
