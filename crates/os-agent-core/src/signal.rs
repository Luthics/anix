//! POSIX signal handling: `SIGINT` (130), `SIGTERM` (143), `SIGUSR1` (compact),
//! `SIGUSR2` (IR JSONL snapshot) (Section 6.3).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::IrStream;

use crate::platform::posix;

const CHILD_KILL_GRACE: Duration = Duration::from_secs(3);

/// Shared atomic signal flags and active child PGID registry.
#[derive(Debug, Clone)]
pub struct SignalState {
    pub sigint: Arc<AtomicBool>,
    pub sigterm: Arc<AtomicBool>,
    pub sigusr1: Arc<AtomicBool>,
    pub sigusr2: Arc<AtomicBool>,
    active_child_pgid: Arc<AtomicI32>,
}

impl SignalState {
    /// Register async-signal-safe handlers for `SIGINT`, `SIGTERM`, `SIGUSR1`, `SIGUSR2`
    /// and ignore `SIGPIPE`.
    pub fn install() -> std::io::Result<Self> {
        posix::ignore_sigpipe();

        let state = Self {
            sigint: Arc::new(AtomicBool::new(false)),
            sigterm: Arc::new(AtomicBool::new(false)),
            sigusr1: Arc::new(AtomicBool::new(false)),
            sigusr2: Arc::new(AtomicBool::new(false)),
            active_child_pgid: Arc::new(AtomicI32::new(0)),
        };

        // Double Ctrl+C guard (registered BEFORE `flag::register` so 1st SIGINT sets `sigint`
        // for graceful child reaping, and 2nd SIGINT immediately SIGKILLs the active child PGID
        // and exits 130 without ever hanging).
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

        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&state.sigint))?;
        signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&state.sigterm))?;
        signal_hook::flag::register(signal_hook::consts::SIGUSR1, Arc::clone(&state.sigusr1))?;
        signal_hook::flag::register(signal_hook::consts::SIGUSR2, Arc::clone(&state.sigusr2))?;

        Ok(state)
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

    /// Gracefully terminate and reap `child`'s process group (`SIGTERM` → `try_wait` up to 3s → `SIGKILL`).
    /// Reaping inside the wait loop ensures Linux zombies exit in ~20ms instead of stalling 3s.
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

/// Atomically dump the current in-memory Canonical IR JSONL snapshot on `SIGUSR2`
/// to `/tmp/agent_<pid>_<epoch_ms>.ctx.jsonl` (mode `0600`) and report the path to `fd 2`.
pub fn dump_ir_snapshot(stream: &IrStream) -> std::io::Result<PathBuf> {
    let jsonl = stream
        .to_jsonl()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let path = PathBuf::from(format!(
        "/tmp/agent_{}_{}.ctx.jsonl",
        std::process::id(),
        posix::timestamp_epoch_ms()
    ));
    posix::write_atomic_0600(&path, jsonl.as_bytes())?;
    eprintln!("{}", path.display());
    Ok(path)
}
