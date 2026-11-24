//! Standard POSIX exit codes for `agent` and helper binaries (Section 6.4).

use std::process::ExitCode as StdExitCode;

/// Normative OS Agent exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExitCode {
    /// Task completed cleanly; payload flushed to `fd 1`.
    Success = 0,
    /// General runtime error (Provider network/protocol failure, I/O error, `flock` lock contention, etc.).
    RuntimeError = 1,
    /// CLI argument, Frontmatter, `config.toml`, or incompatible `ir_version` error.
    UsageError = 2,
    /// Permission / approval gate denial (`approval=ask` without `/dev/tty`, untrusted hooks/bin).
    PermissionDenied = 42,
    /// Budget or timeout limit reached (`max_turns`, `max_tokens`, Provider timeout; or `msh` command timeout).
    TimeoutOrBudget = 124,
    /// Interrupted by `SIGINT` (`128 + 2`).
    SigInt = 130,
    /// Terminated by `SIGTERM` (`128 + 15`).
    SigTerm = 143,
}

impl ExitCode {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<ExitCode> for StdExitCode {
    fn from(code: ExitCode) -> Self {
        StdExitCode::from(code.as_u8())
    }
}
