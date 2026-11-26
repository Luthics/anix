//! Structured JSONL Trace envelope (`--trace-fd=<N>` / `$XDG_RUNTIME_DIR/anix/journal.sock`, Section 8.4).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Trace verbosity level controlled via `ANIX_TRACE_LEVEL` (Section 8.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceLevel {
    #[default]
    Events,
    Deltas,
    Wire,
}

/// Active context window watermark reported in `cycle_end` and `run_end` Trace events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWatermark {
    pub tokens: u64,
    pub max_tokens: u32,
}

/// Common JSONL trace envelope written to `--trace-fd=<N>` or `journal.sock`.
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
