//! Standard POSIX / BSD `sysexits.h` exit codes for `anix` and helper binaries (Section 6.5).

use std::process::ExitCode as StdExitCode;

/// Normative Anix v1.1 exit codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExitCode {
    /// `EX_OK`: Task completed cleanly; payload flushed to `fd 1`.
    Success = 0,
    /// General runtime error (Provider network/protocol failure, I/O error, WAL corruption, `flock` contention).
    RuntimeError = 1,
    /// `EX_USAGE`: CLI argument, `--trace-fd`, Frontmatter, `config.toml`, or incompatible `ir_version` error.
    UsageError = 2,
    /// `EX_TEMPFAIL` (replaces v1.0 `42`): Suspended / awaiting external intervention
    /// (`approval=ask` without `/dev/tty`, `--fail-on-deny`, `[approval].bin` Fail-Closed, untrusted hooks/bins).
    TempFail = 75,
    /// `EX_TIMEOUT`: Budget or timeout limit reached (`max_turns`, `max_tokens`, Provider timeout; or `msh` command timeout).
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
