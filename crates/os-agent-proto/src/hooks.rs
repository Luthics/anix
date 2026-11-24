//! Lifecycle Hook JSON wire payloads (`post-tool.d/` and `on-exit.d/`, Section 9.3).

use serde::{Deserialize, Serialize};

use crate::provider::TokenUsage;

/// Payload passed on `stdin` and expected on `stdout` for `post-tool.d/` hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostToolPayload {
    pub turn_id: u64,
    pub tool_call_id: String,
    pub cmd: String,
    pub exit_code: i32,
    pub output: String,
}

/// Active context window watermark reported to `on-exit.d/` and `run_end` Trace (Sections 9.3 & 9.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWatermark {
    pub tokens: u64,
    pub max_tokens: u32,
}

/// Payload passed on `stdin` for `on-exit.d/` hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnExitPayload {
    pub exit_code: i32,
    pub turns: u32,
    pub usage: TokenUsage,
    pub session_usage: TokenUsage,
    pub context: ContextWatermark,
    pub context_file: String,
}
