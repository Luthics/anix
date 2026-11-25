//! Provider Adapter wire schemas: `coprocess` request envelope, `oneshot` mode,
//! and streaming stdout JSONL events (Section 9.2).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::IR_VERSION;
use crate::ir::{HeaderTag, IrHeader, IrMessage, IrMeta, IrStream, ToolDef};

/// Provider execution mode (v1.1 Section 9.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderMode {
    /// Persistent coprocess reusing HTTP/2 connection pools and TLS sessions across turns (default).
    #[default]
    Coprocess,
    /// Legacy v1.0 single-shot invocation (Canonical IR JSONL + EOF per turn).
    Oneshot,
}

impl ProviderMode {
    pub fn parse_str(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "coprocess" => Ok(Self::Coprocess),
            "oneshot" => Ok(Self::Oneshot),
            other => Err(format!("expected `coprocess` or `oneshot`, got `{other}`")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Coprocess => "coprocess",
            Self::Oneshot => "oneshot",
        }
    }
}

/// Fixed literal `"request"` tag for [`ProviderRequestEnvelope`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestTag {
    #[default]
    Request,
}

/// Header section embedded inside a single-line `coprocess` [`ProviderRequestEnvelope`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestHeader {
    pub tools: Vec<ToolDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
    pub meta: IrMeta,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl From<&IrHeader> for RequestHeader {
    fn from(h: &IrHeader) -> Self {
        Self {
            tools: h.tools.clone(),
            max_output_tokens: h.max_output_tokens,
            params: h.params.clone(),
            meta: h.meta.clone(),
            extra: h.extra.clone(),
        }
    }
}

/// Single-line JSON request envelope written to a `coprocess` Provider's `stdin` (v1.1 Section 9.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderRequestEnvelope {
    pub ir_version: u32,
    #[serde(rename = "type")]
    pub record_type: RequestTag,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_sec: Option<u64>,
    pub header: RequestHeader,
    pub messages: Vec<IrMessage>,
}

impl ProviderRequestEnvelope {
    pub fn from_stream(
        stream: &IrStream,
        request_id: impl Into<String>,
        model: impl Into<String>,
        timeout_sec: u64,
    ) -> Self {
        Self {
            ir_version: IR_VERSION,
            record_type: RequestTag::Request,
            request_id: Some(request_id.into()),
            model: Some(model.into()),
            timeout_sec: Some(timeout_sec),
            header: RequestHeader::from(&stream.header),
            messages: stream.messages.clone(),
        }
    }

    pub fn into_ir_stream(self) -> IrStream {
        IrStream {
            header: IrHeader {
                ir_version: self.ir_version,
                record_type: HeaderTag::Header,
                tools: self.header.tools,
                max_output_tokens: self.header.max_output_tokens,
                params: self.header.params,
                meta: self.header.meta,
                extra: self.header.extra,
            },
            messages: self.messages,
        }
    }
}

/// Standard non-retryable error code when an upstream model rejects image/media inputs (v1.1 §5.5).
pub const ERROR_CODE_UNSUPPORTED_MEDIA: &str = "unsupported_media";

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
