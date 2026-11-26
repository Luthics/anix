//! Append-Only Tree WAL JSONL schema for `-c <file>` / `--leaf <id>` persistence (Section 6.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ir::{ApprovalMode, MediaAttachment, OutputMode, StderrMode, ToolCall};
use crate::provider::TokenUsage;

/// Contract-level configuration recorded in `session_init`, `checkpoint`, and `runtime_update`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalConfigSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<StderrMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
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

/// Single line in an Append-Only Tree WAL JSONL context file (v0.3.0).
///
/// All tree node fields (`id`, `parent_id`, `pruned_usage`) are optional on deserialization so
/// legacy pre-v0.3 linear WAL files remain 100% readable and resumable in place.
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
            | Self::Message { id, .. }
            | Self::RuntimeUpdate { id, .. }
            | Self::Fold { id, .. }
            | Self::Checkpoint { id, .. } => *id,
        }
    }

    /// Return the parent tree node ID if present (`SessionInit` is always root -> `None`).
    pub fn parent_id(&self) -> Option<u64> {
        match self {
            Self::SessionInit { .. } => None,
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
            ts: "2026-11-26T12:00:00Z".into(),
            session_id: Some("550e8400-e29b-41d4-a716-446655440000".into()),
            config: WalConfigSnapshot {
                output: Some(OutputMode::Reply),
                shell: Some("msh".into()),
                stderr: Some(StderrMode::Auto),
                approval: Some(ApprovalMode::Ask),
                vision: Some(true),
            },
        };
        let raw = serde_json::to_string(&init).expect("serialize wal init");
        assert!(raw.contains("\"type\":\"session_init\""));
        assert!(raw.contains("\"id\":1"));
        assert!(raw.contains("\"session_id\":\"550e8400-e29b-41d4-a716-446655440000\""));
        let parsed: WalEvent = serde_json::from_str(&raw).expect("deserialize wal init");
        assert_eq!(parsed, init);
        assert_eq!(parsed.id(), Some(1));
        assert_eq!(parsed.parent_id(), None);

        let fold = WalEvent::Fold {
            id: Some(6),
            parent_id: Some(3),
            ts: "2026-11-26T12:01:00Z".into(),
            from_turn: 2,
            to_turn: 3,
            summary: "folded dead-end".into(),
            fold_usage: Some(TokenUsage {
                input_tokens: 60,
                output_tokens: 15,
                cached_tokens: Some(0),
                cache_write_tokens: Some(0),
                reasoning_tokens: Some(0),
            }),
            pruned_usage: Some(TokenUsage {
                input_tokens: 500,
                output_tokens: 50,
                cached_tokens: Some(0),
                cache_write_tokens: Some(0),
                reasoning_tokens: Some(0),
            }),
        };
        let fold_raw = serde_json::to_string(&fold).expect("serialize fold");
        let parsed_fold: WalEvent = serde_json::from_str(&fold_raw).expect("deserialize fold");
        assert_eq!(parsed_fold, fold);
        assert_eq!(parsed_fold.id(), Some(6));
        assert_eq!(parsed_fold.parent_id(), Some(3));

        // Legacy pre-v0.3 WAL without `id` / `parent_id` / `session_id` must still deserialize cleanly.
        let legacy = r#"{"ir_version":1,"type":"session_init","ts":"2026-11-24T12:00:00Z","config":{"stderr":"auto"}}"#;
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
