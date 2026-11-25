//! RFC v1.1 runtime conformance through the installed CLI contract and local providers.

mod fd;
#[path = "../common/fixture.rs"]
mod fixture;
#[path = "../common/tempdir.rs"]
mod tempdir;
mod wal;

use tempdir::TempDir;
