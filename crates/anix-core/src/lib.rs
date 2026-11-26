//! Anix core kernel library (`anix`, Layer 1).
//!
//! Pure 3-primitive POSIX Agent kernel (`*.agent` + Tree WAL + Maildir IPC).
//! Each module maps directly to a normative section of `docs/RFC.md`:
//! - [`cli`] & [`frontmatter`] & [`config`]: Sections 2, 3 & 7
//! - [`fd`] & [`maildir`] & [`trace`]: Sections 4, 6 & 8.4
//! - [`wal`] & [`compact`] & [`signal`]: Section 5
//! - [`hooks`] & [`strict_modes`]: Sections 4.4, 7.2 & 8.3
//! - [`provider`] & [`runtime`]: Sections 6 & 8
//! - [`platform`] & [`doctor`]: `docs/RUNTIME.md` §8

pub mod cli;
pub mod compact;
pub mod config;
pub mod doctor;
pub mod fd;
pub mod frontmatter;
pub mod hooks;
pub mod maildir;
pub mod platform;
pub mod provider;
pub mod runtime;
pub mod signal;
pub mod strict_modes;
pub mod trace;
pub mod wal;
