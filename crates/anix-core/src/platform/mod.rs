//! Cross-platform POSIX abstractions, pure event-driven wait primitives (`pidfd`/`epoll` on Linux,
//! `kqueue` on macOS/BSD), and Linux ≥ 5.13 kernel enhancements (v1.1 §§6.3 & 10.2).

use std::io;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::process::{Child, ExitStatus};
use std::time::Instant;

pub mod posix;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(not(target_os = "linux"))]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux::{DirWatcher, PreparedSandbox};

#[cfg(not(target_os = "linux"))]
pub use macos::{DirWatcher, PreparedSandbox};

/// Landlock sandbox enforcement policy (Section 10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxPolicy {
    #[default]
    Off,
    BestEffort,
    Strict,
}

/// Detected OS runtime capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub dev_fd_available: bool,
    pub pidfd_supported: bool,
    pub landlock_abi: Option<u32>,
}

/// Result of an event-driven child process wait (v1.1 §6.3).
#[derive(Debug)]
pub enum ChildWaitOutcome {
    Exited(ExitStatus),
    TimedOut,
    SignalWakeup,
}

/// Event-driven wait for `child` to exit, `deadline` to expire, or `sig_fd` to become readable.
///
/// Uses `pidfd_open + epoll` on Linux and `kqueue (EVFILT_PROC / NOTE_EXIT)` on macOS/BSD
/// with exact remaining timeout (zero fixed-interval polling, v1.1 §6.3).
pub fn wait_child_until(
    child: &mut Child,
    deadline: Option<Instant>,
    sig_fd: Option<RawFd>,
) -> io::Result<ChildWaitOutcome> {
    #[cfg(target_os = "linux")]
    {
        linux::wait_child_until(child, deadline, sig_fd)
    }
    #[cfg(not(target_os = "linux"))]
    {
        macos::wait_child_until(child, deadline, sig_fd)
    }
}

/// Probe runtime capabilities cleanly without mutating process state.
pub fn probe_capabilities() -> PlatformCapabilities {
    #[cfg(target_os = "linux")]
    {
        linux::probe_capabilities()
    }
    #[cfg(not(target_os = "linux"))]
    {
        macos::probe_capabilities()
    }
}

/// Prepare Landlock rules (if enabled) in parent process prior to `fork()`.
pub fn prepare_sandbox(
    policy: SandboxPolicy,
    cwd: &Path,
    runtime_dir: &Path,
) -> Result<Option<PreparedSandbox>, String> {
    #[cfg(target_os = "linux")]
    {
        linux::prepare_sandbox(policy, cwd, runtime_dir)
    }
    #[cfg(not(target_os = "linux"))]
    {
        macos::prepare_sandbox(policy, cwd, runtime_dir)
    }
}

/// Apply prepared sandbox inside child `pre_exec`.
///
/// # Safety
/// Must be called after `fork()` in child process prior to `execve()`.
pub unsafe fn apply_prepared_sandbox_in_child(sb: &PreparedSandbox) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        // SAFETY: Delegated to Linux Landlock syscalls in child context.
        unsafe { linux::apply_prepared_sandbox_in_child(sb) }
    }
    #[cfg(not(target_os = "linux"))]
    {
        // SAFETY: No-op on non-Linux.
        unsafe { macos::apply_prepared_sandbox_in_child(sb) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC §10.2-03 — 检查 /dev/fd 可用；Linux 测试主机还须报告 pidfd 支持及 Landlock ABI ≥ 1。
    #[test]
    fn probes_platform_capabilities() {
        let caps = probe_capabilities();
        assert!(
            caps.dev_fd_available,
            "/dev/fd must exist on POSIX platforms"
        );
        #[cfg(target_os = "linux")]
        {
            assert!(
                caps.pidfd_supported,
                "Linux >= 5.13 should support pidfd_open"
            );
            assert!(
                caps.landlock_abi.unwrap_or(0) >= 1,
                "Expected Landlock ABI >= 1 on Linux test host"
            );
        }
    }
}
