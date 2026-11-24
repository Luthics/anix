//! Cross-platform POSIX abstractions and Linux ≥ 5.13 kernel enhancements (Section 10.2).

use std::path::Path;

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
pub unsafe fn apply_prepared_sandbox_in_child(sb: &PreparedSandbox) -> std::io::Result<()> {
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

    // RFC §10.2
    #[test]
    fn probes_platform_capabilities() {
        let caps = probe_capabilities();
        assert!(
            caps.dev_fd_available,
            "/dev/fd must exist on POSIX platforms"
        );
        // §10.2 explicitly supports hosts where pidfd or Landlock is unavailable.
        // If an ABI is reported, it must designate a supported, positive version.
        assert_ne!(caps.landlock_abi, Some(0));
    }
}
