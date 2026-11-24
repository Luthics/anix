//! Lifecycle Hook JSON wire payloads (`post-tool.d/` and `on-exit.d/`, Section 9.3).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Payload passed on `stdin` and expected on `stdout` for `post-tool.d/` hooks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostToolPayload {
    pub turn_id: u64,
    pub tool_call_id: String,
    pub cmd: String,
    pub exit_code: i32,
    pub output: String,
}

/// Payload passed on `stdin` for `on-exit.d/` hooks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnExitPayload {
    pub exit_code: i32,
    pub turns: u32,
    pub usage: Value,
    pub context_file: String,
}
