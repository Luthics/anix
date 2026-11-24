//! Provider Adapter stdout JSONL event stream schema (Section 9.2).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Unified 5-field inclusive token usage shared across Provider events, WAL, Trace, and `on-exit.d/`
/// (Sections 4.1, 6.1, 6.2, 9.2, 9.3, 9.5; strictly excluded from Canonical IR).
///
/// - `input_tokens` MUST be inclusive of `cached_tokens` and `cache_write_tokens`.
/// - `output_tokens` MUST be inclusive of `reasoning_tokens`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u64>,
}

impl TokenUsage {
    pub fn cached(&self) -> u64 {
        self.cached_tokens.unwrap_or(0)
    }

    pub fn cache_write(&self) -> u64 {
        self.cache_write_tokens.unwrap_or(0)
    }

    pub fn reasoning(&self) -> u64 {
        self.reasoning_tokens.unwrap_or(0)
    }

    /// Accumulate another [`TokenUsage`], treating omitted optional fields as `0` (Section 9.2).
    pub fn add_assign(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cached_tokens = Some(self.cached().saturating_add(other.cached()));
        self.cache_write_tokens = Some(self.cache_write().saturating_add(other.cache_write()));
        self.reasoning_tokens = Some(self.reasoning().saturating_add(other.reasoning()));
    }

    /// Return a copy with all 5 fields explicitly populated (omitted optional fields become `Some(0)`).
    pub fn normalized_5(&self) -> Self {
        Self {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cached_tokens: Some(self.cached()),
            cache_write_tokens: Some(self.cache_write()),
            reasoning_tokens: Some(self.reasoning()),
        }
    }

    pub fn is_zero(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.cached() == 0
            && self.cache_write() == 0
            && self.reasoning() == 0
    }
}

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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_write_tokens: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_tokens: Option<u64>,
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

    // RFC §9.2
    #[test]
    fn provider_events_round_trip() {
        let evt = ProviderEvent::Usage {
            input_tokens: 4200,
            output_tokens: 380,
            cached_tokens: Some(3600),
            cache_write_tokens: Some(400),
            reasoning_tokens: Some(200),
        };
        let s = serde_json::to_string(&evt).expect("serialize provider event");
        let parsed: ProviderEvent = serde_json::from_str(&s).expect("deserialize provider event");
        assert_eq!(parsed, evt);
    }
}
