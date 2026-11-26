//! Shared wire-protocol types for Anix (`ir_version = 1`, v0.4.0).
//!
//! This crate intentionally carries zero HTTP, async, or OS-specific runtime
//! dependencies so `anix` and companion tools share a single deterministic source
//! of truth for Canonical IR, Tree WAL, Maildir envelopes, `$ANIX_CTL`, Trace, and exit codes.

pub mod ctl;
pub mod exit_code;
pub mod ir;
pub mod maildir;
pub mod provider;
pub mod trace;
pub mod wal;

/// Normative Canonical IR / WAL protocol version.
pub const IR_VERSION: u32 = 1;

/// Trace envelope schema version (Section 8.4).
pub const TRACE_VERSION: u32 = 1;
