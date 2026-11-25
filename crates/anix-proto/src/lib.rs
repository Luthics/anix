//! Shared wire-protocol types for Anix (`ir_version = 1`).
//!
//! This crate intentionally carries zero HTTP, async, or OS-specific runtime
//! dependencies so `anix`, `msh`, Provider adapters, and `a*` tools share a
//! single deterministic source of truth for all JSONL schemas and exit codes.

pub mod ctl;
pub mod exit_code;
pub mod hooks;
pub mod ir;
pub mod provider;
pub mod trace;
pub mod wal;

/// Normative Canonical IR / WAL protocol version (RFC v1.0).
pub const IR_VERSION: u32 = 1;

/// Trace envelope schema version (Section 9.5).
pub const TRACE_VERSION: u32 = 1;
