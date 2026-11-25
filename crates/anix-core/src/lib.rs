//! Anix core runtime library (`anix`).
//!
//! Each module maps directly to a normative section of `docs/v1.2.0-RFC.md`:
//! - [`cli`] & [`frontmatter`] & [`config`]: Sections 3 & 8
//! - [`fd`] & [`trace`]: Sections 4 & 9.5
//! - [`approval`] & [`vision`]: Section 5
//! - [`wal`] & [`compact`] & [`signal`]: Section 6
//! - [`hooks`] & [`trust`]: Sections 4.4, 8.3, 9.3, 9.4, 9.6
//! - [`provider`] & [`runtime`]: Sections 9.1 & 9.2
//! - [`platform`] & [`doctor`]: Section 10

pub mod approval;
pub mod cli;
pub mod compact;
pub mod config;
pub mod doctor;
pub mod fd;
pub mod frontmatter;
pub mod hooks;
pub mod platform;
pub mod provider;
pub mod runtime;
pub mod signal;
pub mod trace;
pub mod trust;
pub mod vision;
pub mod wal;
