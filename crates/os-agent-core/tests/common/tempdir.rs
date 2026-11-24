use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

use uuid::Uuid;

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> Self {
        Self::under(&std::env::temp_dir())
    }

    pub fn under(parent: &std::path::Path) -> Self {
        let path = parent.join(format!("os-agent-test-{}", Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
