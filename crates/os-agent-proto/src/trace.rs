//! Structured JSONL Trace envelope (`fd 3` / `$XDG_RUNTIME_DIR/agent/journal.sock`, Section 9.5).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Trace verbosity level controlled via `AGENT_TRACE_LEVEL` (Section 9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceLevel {
    #[default]
    Events,
    Deltas,
    Wire,
}

/// Common JSONL trace envelope written to `fd 3` or `journal.sock`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEnvelope {
    pub v: u32,
    pub ts: String,
    pub pid: u32,
    pub ppid: u32,
    pub agent: String,
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(flatten)]
    pub fields: serde_json::Map<String, Value>,
}
