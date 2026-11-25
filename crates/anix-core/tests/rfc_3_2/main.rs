//! Conformance tests for docs/v1.1.0-RFC.md §3.2, rules 1–4.
//! Run with `cargo test -p anix-core --test rfc_3_2`.
//!
//! Issue #1 tracks the byte representation of line endings and blank edges in
//! scripts without frontmatter. Layout cases compare logical lines there.

mod cli;
mod encoding;
mod grammar;

#[path = "../common/tempdir.rs"]
mod tempdir;
use tempdir::TempDir;
#[path = "../common/fixture.rs"]
mod fixture;
