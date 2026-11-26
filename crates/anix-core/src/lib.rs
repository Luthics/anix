//! Anix core kernel library (`anix`, Layer 1).
//!
//! Each module maps directly to a normative section of `docs/RFC.md` (with reference
//! runtime packaging & companion tools documented in `docs/RUNTIME.md`):
//! - [`cli`] & [`frontmatter`] & [`config`]: Sections 2, 3 & 8
//! - [`fd`] & [`trace`]: Sections 4, 7 & 9.4
//! - [`approval`] & [`vision`]: Section 5
//! - [`wal`] & [`compact`] & [`signal`]: Section 6
//! - [`hooks`] & [`trust`]: Sections 4.4, 8.3 & 9.3
//! - [`provider`] & [`runtime`]: Sections 9.1 & 9.2
//! - [`platform`] & [`doctor`]: `docs/RUNTIME.md` §7

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
