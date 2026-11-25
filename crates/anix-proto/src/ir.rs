//! Canonical IR v1 JSONL wire schema (Section 9.1).
//!
//! Every Canonical IR stream is strict UTF-8 JSONL:
//! - Line 1: [`IrHeader`] (`type = "header"`, `ir_version = 1`)
//! - Lines 2..N: [`IrMessage`] (`role = "system" | "user" | "assistant" | "tool"`)

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::IR_VERSION;

/// Runtime `stderr` presentation mode (Section 3.3 / 4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StderrMode {
    #[default]
    Auto,
    Explicit,
}

/// Interactive approval policy (Section 3.3 / 5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalMode {
    #[default]
    Ask,
    Auto,
    Deny,
}

/// Tool schema exposed to a Provider in the IR header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Optional runtime metadata attached to [`IrHeader`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrMeta {
    pub stderr_mode: StderrMode,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_budget: Option<u32>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Line 1 of every Canonical IR v1 JSONL stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrHeader {
    pub ir_version: u32,
    #[serde(rename = "type")]
    pub record_type: HeaderTag,
    pub tools: Vec<ToolDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Value>,
    pub meta: IrMeta,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Fixed literal `"header"` tag for [`IrHeader`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeaderTag {
    #[default]
    Header,
}

/// Tool invocation emitted by an assistant turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

/// Lazy-read local media attachment referenced by IR/WAL (never inline Base64).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub mime: String,
    pub path: String,
}

/// Allowed MIME types for [`MediaAttachment`] (Section 9.1).
pub const ALLOWED_MEDIA_MIMES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Flattened Canonical IR v1 message line (lines 2..N).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum IrMessage {
    System {
        turn_id: u64,
        text: String,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    User {
        turn_id: u64,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        media: Vec<MediaAttachment>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Assistant {
        turn_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ToolCall>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        provider_opaque: Option<Value>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
    Tool {
        turn_id: u64,
        tool_call_id: String,
        #[serde(default, skip_serializing_if = "is_false")]
        is_error: bool,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        media: Vec<MediaAttachment>,
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    },
}

impl IrMessage {
    pub fn turn_id(&self) -> u64 {
        match self {
            Self::System { turn_id, .. }
            | Self::User { turn_id, .. }
            | Self::Assistant { turn_id, .. }
            | Self::Tool { turn_id, .. } => *turn_id,
        }
    }

    pub fn system(turn_id: u64, text: impl Into<String>) -> Self {
        Self::System {
            turn_id,
            text: text.into(),
            extra: BTreeMap::new(),
        }
    }

    pub fn user(turn_id: u64, text: impl Into<String>) -> Self {
        Self::User {
            turn_id,
            text: text.into(),
            media: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

/// Complete Canonical IR v1 JSONL stream (header + ordered messages).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrStream {
    pub header: IrHeader,
    pub messages: Vec<IrMessage>,
}

/// Errors encountered while parsing or validating a Canonical IR v1 stream.
#[derive(Debug, Error)]
pub enum IrError {
    #[error("empty Canonical IR stream (missing header line)")]
    EmptyStream,
    #[error("unsupported ir_version {0} (expected {IR_VERSION})")]
    UnsupportedVersion(u32),
    #[error("invalid IR header on line 1: {0}")]
    InvalidHeader(#[source] serde_json::Error),
    #[error("invalid IR message on line {line}: {source}")]
    InvalidMessage {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("IR invariant violation: {0}")]
    Invariant(String),
}

impl IrStream {
    /// Parse a strict UTF-8 JSONL Canonical IR v1 stream.
    pub fn from_jsonl(input: &str) -> Result<Self, IrError> {
        let mut lines = input
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty());

        let (_, first_line) = lines.next().ok_or(IrError::EmptyStream)?;
        let header: IrHeader = serde_json::from_str(first_line).map_err(IrError::InvalidHeader)?;
        if header.ir_version != IR_VERSION {
            return Err(IrError::UnsupportedVersion(header.ir_version));
        }

        let mut messages = Vec::new();
        for (idx, line) in lines {
            let msg: IrMessage =
                serde_json::from_str(line).map_err(|source| IrError::InvalidMessage {
                    line: idx + 1,
                    source,
                })?;
            messages.push(msg);
        }

        Ok(Self { header, messages })
    }

    /// Serialize the Canonical IR v1 stream to UTF-8 JSONL (`\n`-terminated).
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        let mut out = String::new();
        out.push_str(&serde_json::to_string(&self.header)?);
        out.push('\n');
        for msg in &self.messages {
            out.push_str(&serde_json::to_string(msg)?);
            out.push('\n');
        }
        Ok(out)
    }

    /// Returns `true` if any message in this stream carries one or more media attachments.
    pub fn has_media(&self) -> bool {
        self.messages.iter().any(|m| match m {
            IrMessage::User { media, .. } | IrMessage::Tool { media, .. } => !media.is_empty(),
            IrMessage::System { .. } | IrMessage::Assistant { .. } => false,
        })
    }

    /// Enforce Section 9.1 structural invariants before dispatching to a Provider.
    pub fn validate_invariants(&self) -> Result<(), IrError> {
        if self.header.ir_version != IR_VERSION {
            return Err(IrError::UnsupportedVersion(self.header.ir_version));
        }
        if self.messages.is_empty() {
            return Err(IrError::Invariant(
                "stream must contain at least one message after header".into(),
            ));
        }

        let mut i = 0;
        let mut prev_turn_id: Option<u64> = None;
        while i < self.messages.len() {
            let msg = &self.messages[i];
            let tid = msg.turn_id();
            if let Some(prev) = prev_turn_id
                && tid < prev
            {
                return Err(IrError::Invariant(format!(
                    "turn_id decreased from {prev} to {tid}"
                )));
            }
            prev_turn_id = Some(tid);

            match msg {
                IrMessage::Assistant {
                    text, tool_calls, ..
                } => {
                    let has_text = text.as_deref().is_some_and(|t| !t.is_empty());
                    if !has_text && tool_calls.is_empty() {
                        return Err(IrError::Invariant(format!(
                            "assistant turn {tid} has neither text nor tool_calls"
                        )));
                    }
                    if !tool_calls.is_empty() {
                        let expected: HashSet<&str> =
                            tool_calls.iter().map(|tc| tc.id.as_str()).collect();
                        let mut seen: HashSet<&str> = HashSet::with_capacity(expected.len());
                        let mut j = i + 1;
                        while j < self.messages.len() && seen.len() < expected.len() {
                            match &self.messages[j] {
                                IrMessage::Tool {
                                    turn_id: tool_tid,
                                    tool_call_id,
                                    ..
                                } => {
                                    if *tool_tid < prev_turn_id.unwrap_or(0) {
                                        return Err(IrError::Invariant(format!(
                                            "tool turn_id decreased to {tool_tid}"
                                        )));
                                    }
                                    prev_turn_id = Some(*tool_tid);
                                    if !expected.contains(tool_call_id.as_str()) {
                                        return Err(IrError::Invariant(format!(
                                            "unexpected tool_call_id `{tool_call_id}` after assistant turn {tid}"
                                        )));
                                    }
                                    if !seen.insert(tool_call_id.as_str()) {
                                        return Err(IrError::Invariant(format!(
                                            "duplicate tool_call_id `{tool_call_id}` after assistant turn {tid}"
                                        )));
                                    }
                                    j += 1;
                                }
                                _ => {
                                    return Err(IrError::Invariant(format!(
                                        "non-tool message interleaved before all tool_calls from assistant turn {tid} were closed"
                                    )));
                                }
                            }
                        }
                        if seen.len() != expected.len() {
                            return Err(IrError::Invariant(format!(
                                "unclosed tool_calls on assistant turn {tid}"
                            )));
                        }
                        i = j;
                        continue;
                    }
                }
                IrMessage::Tool { tool_call_id, .. } => {
                    return Err(IrError::Invariant(format!(
                        "orphan tool message `{tool_call_id}` without preceding assistant tool_calls"
                    )));
                }
                _ => {}
            }
            i += 1;
        }

        Ok(())
    }

    /// Return the default `msh` tool definition (v1.1.2 §§7.1.5 & 9.1).
    pub fn msh_tool_def() -> ToolDef {
        ToolDef {
            name: "msh".into(),
            description: "Execute a non-interactive shell command. If it exceeds timeout_sec, it automatically yields to background (unless kill_on_timeout=true) and returns pid/pgid/log path; set timeout_sec=0 to start in background immediately. Redirect final deliverables to $ANIX_OUT and human-facing messages to $ANIX_ERR.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "cmd": {
                        "type": "string",
                        "description": "POSIX shell command to execute."
                    },
                    "timeout_sec": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Max seconds to wait in foreground before yielding to background (0 = start in background immediately; omitted = default)."
                    },
                    "kill_on_timeout": {
                        "type": "boolean",
                        "description": "If true, terminate the process group instead of yielding to background when timeout_sec expires (default: false)."
                    }
                },
                "required": ["cmd"]
            }),
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ir_jsonl_header_and_messages_round_trip() {
        let stream = IrStream {
            header: IrHeader {
                ir_version: IR_VERSION,
                record_type: HeaderTag::Header,
                tools: vec![IrStream::msh_tool_def()],
                max_output_tokens: Some(16384),
                params: BTreeMap::from([("reasoning_effort".into(), Value::String("high".into()))]),
                meta: IrMeta {
                    stderr_mode: StderrMode::Auto,
                    cwd: "/tmp".into(),
                    turn_budget: Some(20),
                    extra: BTreeMap::new(),
                },
                extra: BTreeMap::new(),
            },
            messages: vec![
                IrMessage::system(0, "sys"),
                IrMessage::user(1, "hello"),
                IrMessage::Assistant {
                    turn_id: 2,
                    text: None,
                    reasoning: None,
                    tool_calls: vec![ToolCall {
                        id: "call_1".into(),
                        name: "msh".into(),
                        arguments: serde_json::json!({"cmd": "ls"}),
                    }],
                    provider_opaque: None,
                    extra: BTreeMap::new(),
                },
                IrMessage::Tool {
                    turn_id: 3,
                    tool_call_id: "call_1".into(),
                    is_error: false,
                    text: "[t3] exit=0\n".into(),
                    media: vec![MediaAttachment {
                        mime: "image/png".into(),
                        path: "/tmp/aview_1.png".into(),
                    }],
                    extra: BTreeMap::new(),
                },
            ],
        };

        stream.validate_invariants().expect("valid invariants");
        let jsonl = stream.to_jsonl().expect("serialize ir");
        let parsed = IrStream::from_jsonl(&jsonl).expect("parse ir");
        assert_eq!(parsed, stream);
    }
}
