//! Shared `$AGENT_CTL` (`fd 6`) side-channel frame writer for `a*` tools (Section 7.2).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use os_agent_proto::ctl::{CtlFrame, MAX_CTL_FRAME_BYTES};
use thiserror::Error;
use uuid::Uuid;

/// Environment variable pointing to the anonymous pipe write end (`fd 6`).
pub const AGENT_CTL_ENV: &str = "AGENT_CTL";

#[derive(Debug, Error)]
pub enum CtlError {
    #[error("{AGENT_CTL_ENV} is not set (must be invoked inside an `msh` tool session)")]
    MissingEnv,
    #[error("control frame exceeds {MAX_CTL_FRAME_BYTES} byte limit ({0} bytes)")]
    FrameTooLarge(usize),
    #[error("failed to serialize control frame: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to write control frame to {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// Serialize `frame` as a single JSONL line and write it to `$AGENT_CTL`.
pub fn send_ctl_frame(frame: &CtlFrame) -> Result<(), CtlError> {
    let path = std::env::var(AGENT_CTL_ENV).map_err(|_| CtlError::MissingEnv)?;
    let mut line = serde_json::to_string(frame)?;
    line.push('\n');
    if line.len() > MAX_CTL_FRAME_BYTES {
        return Err(CtlError::FrameTooLarge(line.len()));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|source| CtlError::Io {
            path: path.clone(),
            source,
        })?;
    file.write_all(line.as_bytes())
        .map_err(|source| CtlError::Io { path, source })?;
    Ok(())
}

/// Create a new `0600` temporary file under `/tmp/<prefix>_<uuid>.<ext>`.
pub fn create_secure_tmp_file(prefix: &str, ext: &str) -> std::io::Result<(PathBuf, File)> {
    let path = PathBuf::from(format!("/tmp/{prefix}_{}.{ext}", Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    Ok((path, file))
}
