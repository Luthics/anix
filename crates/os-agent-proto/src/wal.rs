//! Append-Only WAL JSONL schema for `-c <file>` context persistence (Section 6.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ir::{ApprovalMode, MediaAttachment, StderrMode, ToolCall};
use crate::provider::TokenUsage;

/// Contract-level configuration recorded in `session_init`, `checkpoint`, and `runtime_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalConfigSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<StderrMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vision: Option<bool>,
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

/// Single line in an Append-Only WAL JSONL context file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WalEvent {
    SessionInit {
        ir_version: u32,
        ts: String,
        config: WalConfigSnapshot,
    },
    Message {
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
        turn_id: u64,
        ts: String,
        content: String,
        config: WalConfigSnapshot,
    },
    Fold {
        ts: String,
        from_turn: u64,
        to_turn: u64,
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fold_usage: Option<TokenUsage>,
    },
    Checkpoint {
        ir_version: u32,
        turn_id: u64,
        ts: String,
        config: WalConfigSnapshot,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        compact_usage: Option<TokenUsage>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cumulative_usage: Option<TokenUsage>,
    },
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
            ir_version: IR_VERSION,
            ts: "2026-11-24T12:00:00Z".into(),
            config: WalConfigSnapshot {
                stderr: Some(StderrMode::Auto),
                approval: Some(ApprovalMode::Ask),
                vision: Some(true),
            },
        };
        let raw = serde_json::to_string(&init).expect("serialize wal init");
        assert!(raw.contains("\"type\":\"session_init\""));
        let parsed: WalEvent = serde_json::from_str(&raw).expect("deserialize wal init");
        assert_eq!(parsed, init);
    }
}
