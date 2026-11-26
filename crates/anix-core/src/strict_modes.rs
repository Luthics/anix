//! POSIX `StrictModes` ownership and permission gate (RFC §7.2).
//!
//! Replaces the legacy SHA-256 `anix trust` registry with deterministic OS-level
//! checks analogous to OpenSSH `StrictModes`:
//! 1. Owner (`st_uid`) must be the current effective user (`geteuid()`) or `root` (`0`).
//! 2. Mode must NOT allow group or world write (`(st_mode & 0o022) == 0`).
//!
//! Violations abort Fail-Closed with exit code `75` (`EX_TEMPFAIL`).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anix_proto::exit_code::ExitCode;
use thiserror::Error;

use crate::platform::posix;

#[derive(Debug, Error)]
pub enum StrictModeError {
    #[error("POSIX StrictModes violation on `{path}`: {reason}")]
    Violation { path: String, reason: String },
}

impl StrictModeError {
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::TempFail
    }
}

/// Verify POSIX `StrictModes` on a single file (if it exists).
/// Missing files are ignored here so the caller produces standard spawn/not-found errors.
pub fn verify_file(path: &Path) -> Result<(), StrictModeError> {
    let Ok(meta) = fs::metadata(path) else {
        return Ok(());
    };
    check_metadata(path, &meta)
}

/// Verify POSIX `StrictModes` on a directory (e.g. `pre-context.d/`).
pub fn verify_dir(dir: &Path) -> Result<(), StrictModeError> {
    let Ok(meta) = fs::metadata(dir) else {
        return Ok(());
    };
    check_metadata(dir, &meta)
}

fn check_metadata(path: &Path, meta: &fs::Metadata) -> Result<(), StrictModeError> {
    let euid = posix::current_uid();
    let uid = meta.uid();
    if uid != euid && uid != 0 {
        return Err(StrictModeError::Violation {
            path: path.display().to_string(),
            reason: format!("owned by uid {uid} (expected current euid {euid} or root 0)"),
        });
    }

    let mode = meta.mode();
    if (mode & 0o022) != 0 {
        return Err(StrictModeError::Violation {
            path: path.display().to_string(),
            reason: format!(
                "unsafe group/world-writable permissions {:04o} (run `chmod go-w`)",
                mode & 0o7777
            ),
        });
    }

    Ok(())
}

/// Returns `true` if `path` is a hook mask (symlink pointing to `/dev/null` or 0-byte regular file).
pub fn is_hook_mask(path: &Path) -> bool {
    if let Ok(target) = fs::read_link(path)
        && target == Path::new("/dev/null")
    {
        return true;
    }
    if let Ok(meta) = fs::metadata(path)
        && meta.is_file()
        && meta.len() == 0
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn rejects_group_or_world_writable_files_and_accepts_0755() {
        let dir = std::env::temp_dir().join(format!("anix-sm-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        let script = dir.join("10-hook");
        fs::write(&script, "#!/bin/sh\ncat\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        verify_dir(&dir).expect("0755 dir passes");
        verify_file(&script).expect("0755 file passes");

        fs::set_permissions(&script, fs::Permissions::from_mode(0o775)).unwrap();
        let err = verify_file(&script).expect_err("0775 must fail");
        assert_eq!(err.exit_code(), ExitCode::TempFail);

        fs::set_permissions(&script, fs::Permissions::from_mode(0o757)).unwrap();
        let err = verify_file(&script).expect_err("0757 must fail");
        assert_eq!(err.exit_code(), ExitCode::TempFail);

        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        let err = verify_dir(&dir).expect_err("0777 dir must fail");
        assert_eq!(err.exit_code(), ExitCode::TempFail);

        let _ = fs::remove_dir_all(&dir);
    }
}
