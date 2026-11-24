//! Provider Adapter stdout JSONL event stream schema (Section 9.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Single JSONL event emitted on a Provider adapter's `stdout`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderEvent {
    ReasoningDelta {
        text: String,
    },
    TextDelta {
        text: String,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },
    Usage {
        input_tokens: u64,
        output_tokens: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cached_tokens: Option<u64>,
    },
    /// Provider-opaque state (e.g., encrypted reasoning signature) persisted on the assistant turn.
    Opaque {
        data: Value,
    },
    Error {
        code: String,
        message: String,
        #[serde(default)]
        retryable: bool,
    },
    Done {
        finish_reason: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_events_round_trip() {
        let evt = ProviderEvent::Usage {
            input_tokens: 4200,
            output_tokens: 180,
            cached_tokens: Some(3800),
        };
        let s = serde_json::to_string(&evt).expect("serialize provider event");
        let parsed: ProviderEvent = serde_json::from_str(&s).expect("deserialize provider event");
        assert_eq!(parsed, evt);
    }
}
