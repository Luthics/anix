//! Conformance tests for docs/v1.0.0-RFC.md §3.2, rules 1–4.
//! Run with `cargo test -p os-agent-core --test rfc_3_2`.
//!
//! Issue #1 tracks the byte representation of line endings and blank edges in
//! scripts without frontmatter. Layout cases compare logical lines there.

mod cli;
mod encoding;
mod grammar;

use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

use uuid::Uuid;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("os-agent-rfc-3-2-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
