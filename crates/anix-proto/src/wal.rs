//! Append-Only Tree WAL JSONL schema for `-c <file>` / `--leaf <id>` / `anix fork` (Section 5.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ir::{MediaAttachment, OutputMode, StderrMode, ToolCall};
use crate::provider::TokenUsage;

/// Contract-level configuration recorded in `session_init`, `branch_base`, `checkpoint`, and
/// `runtime_update`. Intentionally ignores legacy unknown fields (e.g. pre-v0.4 `approval`/`vision`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalConfigSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<StderrMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// Message role stored in WAL `"type": "message"` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Single line in an Append-Only Tree WAL JSONL context file (v0.4.0).
///
/// Tree node fields (`id`, `parent_id`, `pruned_usage`, `message_ids`) are optional on
/// deserialization so legacy pre-v0.3 linear WAL files remain readable and resumable in place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalEvent {
    SessionInit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        ir_version: u32,
        ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        config: WalConfigSnapshot,
    },
    /// Cross-file read-only branch root created by `anix fork <src> --leaf <id> -o <dst>` (v0.4.0).
    BranchBase {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        ir_version: u32,
        /// Parent WAL reference formatted as `<abs_path>#<leaf_id>`.
        base: String,
        ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        config: WalConfigSnapshot,
    },
    Message {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<u64>,
        turn_id: u64,
        role: WalRole,
        ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        is_error: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        media: Vec<MediaAttachment>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_opaque: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        usage: Option<TokenUsage>,
        /// Maildir message IDs committed by this user turn (crash-recovery deduplication, v0.4.0).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        message_ids: Vec<String>,
    },
    RuntimeUpdate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<u64>,
        turn_id: u64,
        ts: String,
        content: String,
        config: WalConfigSnapshot,
    },
    Fold {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<u64>,
        ts: String,
        from_turn: u64,
        to_turn: u64,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fold_usage: Option<TokenUsage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pruned_usage: Option<TokenUsage>,
    },
    Checkpoint {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<u64>,
        ir_version: u32,
        turn_id: u64,
        ts: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        config: WalConfigSnapshot,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compact_usage: Option<TokenUsage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cumulative_usage: Option<TokenUsage>,
    },
}

impl WalEvent {
    /// Return the monotonic tree node ID if present (tree WAL v0.3.0+).
    pub fn id(&self) -> Option<u64> {
        match self {
            Self::SessionInit { id, .. }
            | Self::BranchBase { id, .. }
            | Self::Message { id, .. }
            | Self::RuntimeUpdate { id, .. }
            | Self::Fold { id, .. }
            | Self::Checkpoint { id, .. } => *id,
        }
    }

    /// Return the parent tree node ID if present (`SessionInit` and `BranchBase` are file roots -> `None`).
    pub fn parent_id(&self) -> Option<u64> {
        match self {
            Self::SessionInit { .. } | Self::BranchBase { .. } => None,
            Self::Message { parent_id, .. }
            | Self::RuntimeUpdate { parent_id, .. }
            | Self::Fold { parent_id, .. }
            | Self::Checkpoint { parent_id, .. } => *parent_id,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IR_VERSION;

    #[test]
    fn wal_events_round_trip() {
        let init = WalEvent::SessionInit {
            id: Some(1),
            ir_version: IR_VERSION,
            ts: "2026-11-27T00:00:00Z".into(),
            session_id: Some("550e8400-e29b-41d4-a716-446655440000".into()),
            config: WalConfigSnapshot {
                output: Some(OutputMode::Reply),
                shell: Some("msh".into()),
                tool_name: Some("shell".into()),
                stderr: Some(StderrMode::Auto),
            },
        };
        let raw = serde_json::to_string(&init).expect("serialize wal init");
        let parsed: WalEvent = serde_json::from_str(&raw).expect("deserialize wal init");
        assert_eq!(parsed, init);
        assert_eq!(parsed.id(), Some(1));
        assert_eq!(parsed.parent_id(), None);

        let base = WalEvent::BranchBase {
            id: Some(1),
            ir_version: IR_VERSION,
            base: "/tmp/parent.jsonl#4".into(),
            ts: "2026-11-27T00:01:00Z".into(),
            session_id: Some("sid-fork".into()),
            config: WalConfigSnapshot::default(),
        };
        let base_raw = serde_json::to_string(&base).expect("serialize branch_base");
        let parsed_base: WalEvent =
            serde_json::from_str(&base_raw).expect("deserialize branch_base");
        assert_eq!(parsed_base, base);

        // Legacy pre-v0.4 WAL with `approval`/`vision` and without `id` must still deserialize cleanly.
        let legacy = r#"{"ir_version":1,"type":"session_init","ts":"2026-11-24T12:00:00Z","config":{"stderr":"auto","approval":"ask","vision":true}}"#;
        let parsed_legacy: WalEvent = serde_json::from_str(legacy).expect("deserialize legacy");
        assert!(matches!(
            parsed_legacy,
            WalEvent::SessionInit {
                id: None,
                session_id: None,
                ..
            }
        ));
    }
}
