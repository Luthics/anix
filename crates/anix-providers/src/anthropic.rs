//! Anthropic Messages API (`/v1/messages`) dialect (Sections 9.1 & 9.2).
//!
//! Key protocol guarantees:
//! - Preserves the frozen leading `system` prefix with ephemeral prompt-caching breakpoints
//!   (can be disabled via `params.prompt_caching = false`).
//! - Enforces Anthropic's strict `user` / `assistant` alternation by merging adjacent
//!   same-role blocks (including contiguous `tool_result` batches and degraded `runtime_update`s).
//! - Preserves native multimodal `image` blocks inside `tool_result.content` and `is_error`.
//! - Persists and replays signed `thinking` and `redacted_thinking` blocks via `provider_opaque`.
//! - Normalizes Anthropic's non-inclusive `input_tokens` to RFC Section 9.2 inclusive totals:
//!   `input_tokens_inclusive = input_tokens + cache_read_input_tokens + cache_creation_input_tokens`.

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader};

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, IrStream, MediaAttachment};
use anix_proto::provider::ProviderEvent;
use serde_json::{Map, Value, json};

use crate::common::{
    ProviderEnv, TranslateError, build_http_agent, drive_sse_reader, emit_event,
    parse_tool_arguments, read_media_base64, resolve_session_header, send_post_with_agent,
};

const TAG: &str = "anix-provider-anthropic";

/// Default `anthropic-version` header when not overridden via `params.anthropic_version`.
pub const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default `max_tokens` required by Anthropic `/v1/messages` when `max_output_tokens` is omitted.
const DEFAULT_MAX_TOKENS: u32 = 8192;

/// Namespace key stored inside `ProviderEvent::Opaque.data` / `IrMessage::Assistant.provider_opaque`.
pub const OPAQUE_NAMESPACE: &str = "anthropic";

/// Resolve the Anthropic Messages endpoint URL from `base_url`.
pub fn endpoint_url(base_url: &str) -> String {
    if base_url.ends_with("/messages") {
        base_url.to_owned()
    } else if base_url.ends_with("/v1") {
        format!("{base_url}/messages")
    } else {
        format!("{base_url}/v1/messages")
    }
}

/// Build an Anthropic `/v1/messages` JSON request payload from a Canonical IR v1 stream.
pub fn build_request(stream: &IrStream, model: &str) -> Result<Value, TranslateError> {
    let prompt_caching = stream
        .header
        .params
        .get("prompt_caching")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let mut system_text: Option<String> = None;
    let mut messages: Vec<Value> = Vec::new();

    for msg in &stream.messages {
        match msg {
            IrMessage::System { text, .. } => {
                if system_text.is_none() {
                    // Frozen leading system prefix: preserve byte-for-byte.
                    system_text = Some(text.clone());
                } else {
                    // Trailing runtime_update: Anthropic forbids mid-stream `system` role.
                    push_role_blocks(&mut messages, "user", vec![text_block(text)]);
                }
            }
            IrMessage::User { text, media, .. } => {
                let mut blocks = Vec::new();
                if !text.is_empty() || media.is_empty() {
                    blocks.push(text_block(text));
                }
                for m in media {
                    blocks.push(encode_media_block(m)?);
                }
                push_role_blocks(&mut messages, "user", blocks);
            }
            IrMessage::Assistant {
                text,
                tool_calls,
                provider_opaque,
                ..
            } => {
                let mut blocks = Vec::new();

                // Replay signed `thinking` and `redacted_thinking` blocks verbatim before
                // text / tool_use blocks (required by Anthropic extended thinking).
                if let Some(opaque) = provider_opaque
                    .as_ref()
                    .and_then(|o| o.get(OPAQUE_NAMESPACE))
                    .and_then(|a| a.get("thinking_blocks"))
                    .and_then(Value::as_array)
                {
                    for item in opaque {
                        blocks.push(item.clone());
                    }
                }

                if let Some(t) = text
                    && !t.is_empty()
                {
                    blocks.push(text_block(t));
                }

                for tc in tool_calls {
                    let input = match &tc.arguments {
                        Value::Object(_) => tc.arguments.clone(),
                        Value::Null => json!({}),
                        other => json!({ "raw": other }),
                    };
                    blocks.push(json!({
                        "type": "tool_use",
                        "id": tc.id,
                        "name": tc.name,
                        "input": input
                    }));
                }

                if !blocks.is_empty() {
                    push_role_blocks(&mut messages, "assistant", blocks);
                }
            }
            IrMessage::Tool {
                tool_call_id,
                is_error,
                text,
                media,
                ..
            } => {
                let mut tr = Map::new();
                tr.insert("type".into(), Value::String("tool_result".into()));
                tr.insert("tool_use_id".into(), Value::String(tool_call_id.clone()));
                if *is_error {
                    tr.insert("is_error".into(), Value::Bool(true));
                }
                if media.is_empty() {
                    tr.insert("content".into(), Value::String(text.clone()));
                } else {
                    let mut content_blocks = vec![text_block(text)];
                    for m in media {
                        content_blocks.push(encode_media_block(m)?);
                    }
                    tr.insert("content".into(), Value::Array(content_blocks));
                }
                push_role_blocks(&mut messages, "user", vec![Value::Object(tr)]);
            }
        }
    }

    // Add a trailing ephemeral cache breakpoint on the final user message block when
    // prompt caching is enabled (total breakpoints: 1 system + 1 last tool + 1 last user <= 4).
    if prompt_caching
        && let Some(last_msg) = messages.last_mut()
        && last_msg.get("role").and_then(Value::as_str) == Some("user")
        && let Some(content) = last_msg.get_mut("content").and_then(Value::as_array_mut)
        && let Some(last_block) = content.last_mut().and_then(Value::as_object_mut)
    {
        last_block.insert("cache_control".into(), json!({ "type": "ephemeral" }));
    }

    let mut body = Map::new();
    body.insert("model".into(), Value::String(model.to_owned()));
    body.insert("stream".into(), Value::Bool(true));

    let explicit_max_tokens = stream.header.max_output_tokens.filter(|&v| v >= 1);
    let mut max_tokens = explicit_max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    body.insert("max_tokens".into(), Value::from(max_tokens));

    if let Some(sys) = system_text {
        let mut sys_block = Map::new();
        sys_block.insert("type".into(), Value::String("text".into()));
        sys_block.insert("text".into(), Value::String(sys));
        if prompt_caching {
            sys_block.insert("cache_control".into(), json!({ "type": "ephemeral" }));
        }
        body.insert(
            "system".into(),
            Value::Array(vec![Value::Object(sys_block)]),
        );
    }

    body.insert("messages".into(), Value::Array(messages));

    // Omit `tools` completely when empty (e.g. during full compaction, Section 9.1).
    if !stream.header.tools.is_empty() {
        let mut tools: Vec<Value> = stream
            .header
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();
        if prompt_caching && let Some(last_tool) = tools.last_mut().and_then(Value::as_object_mut) {
            last_tool.insert("cache_control".into(), json!({ "type": "ephemeral" }));
        }
        body.insert("tools".into(), Value::Array(tools));
    }

    let has_explicit_thinking = stream.header.params.contains_key("thinking");

    for (key, val) in &stream.header.params {
        match key.as_str() {
            // Handled via request construction or HTTP headers, not JSON body.
            "prompt_caching" | "anthropic_version" | "anthropic_beta" | "session_header" => {}
            "reasoning_effort" => {
                if !has_explicit_thinking && let Some(effort) = val.as_str() {
                    match effort.to_ascii_lowercase().as_str() {
                        "none" | "off" | "disabled" => {
                            body.insert("thinking".into(), json!({ "type": "disabled" }));
                        }
                        "low" | "medium" | "high" | "xhigh" | "max" => {
                            let target_budget: u32 = match effort.to_ascii_lowercase().as_str() {
                                "low" => 2048,
                                "medium" => 8192,
                                "high" => 16384,
                                _ => 32768,
                            };
                            // Anthropic requires `max_tokens > thinking.budget_tokens` and `budget_tokens >= 1024`.
                            let budget = if explicit_max_tokens.is_none() {
                                if max_tokens <= target_budget {
                                    max_tokens = target_budget.saturating_add(4096);
                                    body.insert("max_tokens".into(), Value::from(max_tokens));
                                }
                                target_budget
                            } else if max_tokens > 1024 {
                                target_budget.min(max_tokens - 1024).max(1024)
                            } else {
                                eprintln!(
                                    "{TAG}: warning: max_output_tokens ({max_tokens}) too small for extended thinking (need > 1024); omitting `thinking`"
                                );
                                continue;
                            };
                            body.insert(
                                "thinking".into(),
                                json!({ "type": "enabled", "budget_tokens": budget }),
                            );
                        }
                        other => {
                            eprintln!(
                                "{TAG}: warning: ignoring unknown `reasoning_effort` value `{other}`"
                            );
                        }
                    }
                }
            }
            "stop" => match val {
                Value::String(s) => {
                    body.insert(
                        "stop_sequences".into(),
                        Value::Array(vec![Value::String(s.clone())]),
                    );
                }
                Value::Array(_) => {
                    body.insert("stop_sequences".into(), val.clone());
                }
                _ => {
                    eprintln!("{TAG}: warning: ignoring invalid `stop` param");
                }
            },
            "extra_body" => {
                if let Some(extra_map) = val.as_object() {
                    for (ek, ev) in extra_map {
                        if matches!(ek.as_str(), "model" | "messages" | "stream" | "tools") {
                            eprintln!("{TAG}: warning: ignoring protected `extra_body.{ek}`");
                            continue;
                        }
                        body.insert(ek.clone(), ev.clone());
                    }
                } else {
                    eprintln!("{TAG}: warning: ignoring non-object `extra_body` param");
                }
            }
            "temperature" | "top_p" | "top_k" | "stop_sequences" | "tool_choice" | "metadata"
            | "service_tier" | "thinking" | "output_config" => {
                body.insert(key.clone(), val.clone());
            }
            other => {
                eprintln!("{TAG}: warning: ignoring unsupported param `{other}`");
            }
        }
    }

    Ok(Value::Object(body))
}

fn text_block(text: &str) -> Value {
    json!({ "type": "text", "text": text })
}

fn encode_media_block(media: &MediaAttachment) -> Result<Value, TranslateError> {
    let b64 = read_media_base64(media)?;
    Ok(json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media.mime,
            "data": b64
        }
    }))
}

/// Push content blocks under `role`, coalescing consecutive same-role messages into a single
/// message to satisfy Anthropic's strict role alternation requirement.
fn push_role_blocks(messages: &mut Vec<Value>, role: &str, mut blocks: Vec<Value>) {
    if let Some(last) = messages.last_mut()
        && last.get("role").and_then(Value::as_str) == Some(role)
        && let Some(arr) = last.get_mut("content").and_then(Value::as_array_mut)
    {
        arr.append(&mut blocks);
        return;
    }
    messages.push(json!({
        "role": role,
        "content": blocks
    }));
}

#[derive(Debug, Default)]
struct PendingToolUse {
    id: String,
    name: String,
    arguments: String,
    initial_input: Option<Value>,
}

#[derive(Debug, Default)]
struct PendingThinking {
    thinking: String,
    signature: String,
}

#[derive(Debug, Default)]
struct RawAnthropicUsage {
    saw_any: bool,
    uncached_input_tokens: u64,
    cache_read_input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    output_tokens: u64,
}

impl RawAnthropicUsage {
    fn merge_from(&mut self, obj: &Value) {
        if !obj.is_object() {
            return;
        }
        self.saw_any = true;
        if let Some(v) = obj.get("input_tokens").and_then(Value::as_u64) {
            self.uncached_input_tokens = v;
        }
        if let Some(v) = obj.get("cache_read_input_tokens").and_then(Value::as_u64) {
            self.cache_read_input_tokens = Some(v);
        }
        if let Some(v) = obj
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
        {
            self.cache_creation_input_tokens = Some(v);
        }
        if let Some(v) = obj.get("output_tokens").and_then(Value::as_u64) {
            self.output_tokens = v;
        }
    }

    /// Normalize Anthropic's non-inclusive `input_tokens` into RFC Section 9.2 inclusive totals.
    fn to_inclusive_event(&self) -> ProviderEvent {
        let cached = self.cache_read_input_tokens.unwrap_or(0);
        let cache_write = self.cache_creation_input_tokens.unwrap_or(0);
        let inclusive_input = self
            .uncached_input_tokens
            .saturating_add(cached)
            .saturating_add(cache_write);
        ProviderEvent::Usage {
            input_tokens: inclusive_input,
            output_tokens: self.output_tokens,
            cached_tokens: self.cache_read_input_tokens,
            cache_write_tokens: self.cache_creation_input_tokens,
            reasoning_tokens: None,
        }
    }
}

#[derive(Debug, Default)]
struct StreamState {
    pending_tools: BTreeMap<usize, PendingToolUse>,
    pending_thinking: BTreeMap<usize, PendingThinking>,
    emitted_call_ids: HashSet<String>,
    opaque_thinking_blocks: Vec<Value>,
    usage: RawAnthropicUsage,
    stop_reason: Option<String>,
    saw_tool_call: bool,
    saw_done: bool,
}

/// Execute a single-shot streaming POST to Anthropic `/v1/messages`.
pub fn execute_streaming(env: &ProviderEnv, stream: &IrStream, body: &Value) -> ExitCode {
    let agent = build_http_agent(env.timeout_sec);
    execute_streaming_with_agent(&agent, env, stream, body, emit_event)
}

/// Testable streaming executor with custom event sink.
pub fn execute_streaming_with_sink<F>(
    env: &ProviderEnv,
    stream: &IrStream,
    body: &Value,
    sink: F,
) -> ExitCode
where
    F: FnMut(&ProviderEvent),
{
    let agent = build_http_agent(env.timeout_sec);
    execute_streaming_with_agent(&agent, env, stream, body, sink)
}

/// Pooled streaming executor reusing an HTTP `ureq::Agent` across coprocess turns.
pub fn execute_streaming_with_agent<F>(
    agent: &ureq::Agent,
    env: &ProviderEnv,
    stream: &IrStream,
    body: &Value,
    mut sink: F,
) -> ExitCode
where
    F: FnMut(&ProviderEvent),
{
    let url = endpoint_url(&env.base_url);
    let anthropic_version = stream
        .header
        .params
        .get("anthropic_version")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_ANTHROPIC_VERSION)
        .to_owned();

    let mut headers: Vec<(String, String)> = vec![("anthropic-version".into(), anthropic_version)];
    if !env.api_key.trim().is_empty() {
        headers.push(("x-api-key".into(), env.api_key.clone()));
    }
    if let Some(h) = resolve_session_header(&stream.header.params, env.session_id.as_deref(), TAG) {
        headers.push(h);
    }
    if let Some(beta_val) = stream.header.params.get("anthropic_beta") {
        let beta = match beta_val {
            Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
            Value::Array(arr) => {
                let joined: Vec<&str> = arr
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                if joined.is_empty() {
                    None
                } else {
                    Some(joined.join(","))
                }
            }
            _ => None,
        };
        if let Some(b) = beta {
            headers.push(("anthropic-beta".into(), b));
        }
    }

    let response = match send_post_with_agent(
        agent,
        TAG,
        &url,
        env.timeout_sec,
        &headers,
        body,
        stream.has_media(),
        &mut sink,
    ) {
        Ok(resp) => resp,
        Err(code) => return code,
    };

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if content_type.contains("application/json") {
        let raw = match response.into_body().read_to_string() {
            Ok(s) => s,
            Err(err) => {
                let msg = format!("failed to read JSON response body: {err}");
                sink(&ProviderEvent::Error {
                    code: "stream_interrupted".into(),
                    message: msg.clone(),
                    retryable: true,
                });
                eprintln!("{TAG}: {msg}");
                return ExitCode::RuntimeError;
            }
        };
        return consume_json_response(&raw, &mut sink);
    }

    let reader = BufReader::new(response.into_body().into_reader());
    consume_sse_stream_with_sink(reader, &mut sink)
}

/// Parse a non-streaming `application/json` Anthropic Messages response (or 200-wrapped error).
pub fn consume_json_response<F>(raw: &str, sink: &mut F) -> ExitCode
where
    F: FnMut(&ProviderEvent),
{
    let payload: Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(err) => {
            let msg = format!("invalid JSON response body: {err}");
            sink(&ProviderEvent::Error {
                code: "invalid_response".into(),
                message: msg.clone(),
                retryable: false,
            });
            eprintln!("{TAG}: {msg}");
            return ExitCode::RuntimeError;
        }
    };

    if payload.get("type").and_then(Value::as_str) == Some("error")
        || payload.get("error").is_some_and(|v| !v.is_null())
    {
        return emit_anthropic_error(&payload, sink);
    }

    let mut state = StreamState::default();
    if let Some(usage) = payload.get("usage") {
        state.usage.merge_from(usage);
    }
    if let Some(sr) = payload.get("stop_reason").and_then(Value::as_str) {
        state.stop_reason = Some(sr.to_owned());
    }

    if let Some(blocks) = payload.get("content").and_then(Value::as_array) {
        for block in blocks {
            match block.get("type").and_then(Value::as_str) {
                Some("text") => {
                    if let Some(t) = block.get("text").and_then(Value::as_str)
                        && !t.is_empty()
                    {
                        sink(&ProviderEvent::TextDelta { text: t.to_owned() });
                    }
                }
                Some("thinking") => {
                    if let Some(t) = block.get("thinking").and_then(Value::as_str)
                        && !t.is_empty()
                    {
                        sink(&ProviderEvent::ReasoningDelta { text: t.to_owned() });
                    }
                    if block
                        .get("signature")
                        .and_then(Value::as_str)
                        .is_some_and(|s| !s.is_empty())
                    {
                        state.opaque_thinking_blocks.push(block.clone());
                    }
                }
                Some("redacted_thinking") => {
                    state.opaque_thinking_blocks.push(block.clone());
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let input = block.get("input").cloned().unwrap_or_else(|| json!({}));
                    if !id.is_empty()
                        && !name.is_empty()
                        && state.emitted_call_ids.insert(id.clone())
                    {
                        sink(&ProviderEvent::ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                        state.saw_tool_call = true;
                    }
                }
                _ => {}
            }
        }
    }

    finalize_stream(&mut state, sink);
    ExitCode::Success
}

/// Consume an Anthropic Messages SSE stream into canonical [`ProviderEvent`]s.
pub fn consume_sse_stream_with_sink<R, F>(reader: R, sink: &mut F) -> ExitCode
where
    R: BufRead,
    F: FnMut(&ProviderEvent),
{
    let mut state = StreamState::default();
    match drive_sse_reader(TAG, reader, sink, |ev, data, s| {
        dispatch_sse_frame(ev, data, &mut state, s)
    }) {
        Ok(Some(code)) => code,
        Ok(None) => {
            if state.saw_done {
                ExitCode::Success
            } else if state.stop_reason.is_some() {
                finalize_stream(&mut state, sink);
                ExitCode::Success
            } else {
                let msg = "upstream SSE stream closed without terminal completion event";
                sink(&ProviderEvent::Error {
                    code: "stream_truncated".into(),
                    message: msg.into(),
                    retryable: true,
                });
                eprintln!("{TAG}: {msg}");
                ExitCode::RuntimeError
            }
        }
        Err(code) => code,
    }
}

fn dispatch_sse_frame<F>(
    sse_event: &str,
    data: &str,
    state: &mut StreamState,
    sink: &mut F,
) -> Option<ExitCode>
where
    F: FnMut(&ProviderEvent),
{
    let Ok(payload) = serde_json::from_str::<Value>(data) else {
        return None;
    };

    let ev_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or(sse_event);

    match ev_type {
        "ping" => {}
        "message_start" => {
            if let Some(usage) = payload.get("message").and_then(|m| m.get("usage")) {
                state.usage.merge_from(usage);
            }
        }
        "content_block_start" => {
            let idx = payload.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(block) = payload.get("content_block") {
                match block.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(Value::as_str)
                            && !t.is_empty()
                        {
                            sink(&ProviderEvent::TextDelta { text: t.to_owned() });
                        }
                    }
                    Some("thinking") => {
                        let thinking = block
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let signature = block
                            .get("signature")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        if !thinking.is_empty() {
                            sink(&ProviderEvent::ReasoningDelta {
                                text: thinking.clone(),
                            });
                        }
                        state.pending_thinking.insert(
                            idx,
                            PendingThinking {
                                thinking,
                                signature,
                            },
                        );
                    }
                    Some("redacted_thinking") => {
                        state.opaque_thinking_blocks.push(block.clone());
                    }
                    Some("tool_use") => {
                        let id = block
                            .get("id")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let initial_input = block
                            .get("input")
                            .filter(|v| v.as_object().is_some_and(|m| !m.is_empty()))
                            .cloned();
                        state.pending_tools.insert(
                            idx,
                            PendingToolUse {
                                id,
                                name,
                                arguments: String::new(),
                                initial_input,
                            },
                        );
                    }
                    _ => {}
                }
            }
        }
        "content_block_delta" => {
            let idx = payload.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(delta) = payload.get("delta") {
                match delta.get("type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(t) = delta.get("text").and_then(Value::as_str)
                            && !t.is_empty()
                        {
                            sink(&ProviderEvent::TextDelta { text: t.to_owned() });
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(t) = delta.get("thinking").and_then(Value::as_str)
                            && !t.is_empty()
                        {
                            let entry = state.pending_thinking.entry(idx).or_default();
                            entry.thinking.push_str(t);
                            sink(&ProviderEvent::ReasoningDelta { text: t.to_owned() });
                        }
                    }
                    Some("signature_delta") => {
                        if let Some(sig) = delta.get("signature").and_then(Value::as_str) {
                            let entry = state.pending_thinking.entry(idx).or_default();
                            entry.signature.push_str(sig);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(partial) = delta.get("partial_json").and_then(Value::as_str)
                            && let Some(entry) = state.pending_tools.get_mut(&idx)
                        {
                            entry.arguments.push_str(partial);
                        }
                    }
                    _ => {}
                }
            }
        }
        "content_block_stop" => {
            let idx = payload.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            if let Some(thinking) = state.pending_thinking.remove(&idx)
                && !thinking.signature.is_empty()
            {
                state.opaque_thinking_blocks.push(json!({
                    "type": "thinking",
                    "thinking": thinking.thinking,
                    "signature": thinking.signature
                }));
            }
            if let Some(tc) = state.pending_tools.remove(&idx) {
                emit_pending_tool(idx, tc, state, sink);
            }
        }
        "message_delta" => {
            if let Some(sr) = payload
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(Value::as_str)
                && !sr.is_empty()
            {
                state.stop_reason = Some(sr.to_owned());
            }
            if let Some(usage) = payload.get("usage") {
                state.usage.merge_from(usage);
            }
        }
        "message_stop" => {
            finalize_stream(state, sink);
            return Some(ExitCode::Success);
        }
        "error" => {
            return Some(emit_anthropic_error(&payload, sink));
        }
        _ => {}
    }

    None
}

fn emit_pending_tool<F>(idx: usize, tc: PendingToolUse, state: &mut StreamState, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    if tc.name.is_empty() {
        return;
    }
    let id = if tc.id.is_empty() {
        format!("toolu_{idx}")
    } else {
        tc.id
    };
    if state.emitted_call_ids.insert(id.clone()) {
        let arguments = if !tc.arguments.trim().is_empty() {
            parse_tool_arguments(&tc.arguments)
        } else {
            tc.initial_input.unwrap_or_else(|| json!({}))
        };
        sink(&ProviderEvent::ToolCall {
            id,
            name: tc.name,
            arguments,
        });
        state.saw_tool_call = true;
    }
}

fn finalize_stream<F>(state: &mut StreamState, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    let remaining_thinking = std::mem::take(&mut state.pending_thinking);
    for (_, thinking) in remaining_thinking {
        if !thinking.signature.is_empty() {
            state.opaque_thinking_blocks.push(json!({
                "type": "thinking",
                "thinking": thinking.thinking,
                "signature": thinking.signature
            }));
        }
    }

    let remaining_tools = std::mem::take(&mut state.pending_tools);
    for (idx, tc) in remaining_tools {
        emit_pending_tool(idx, tc, state, sink);
    }

    if state.usage.saw_any {
        sink(&state.usage.to_inclusive_event());
    }

    if !state.opaque_thinking_blocks.is_empty() {
        let blocks = std::mem::take(&mut state.opaque_thinking_blocks);
        sink(&ProviderEvent::Opaque {
            data: json!({
                OPAQUE_NAMESPACE: {
                    "thinking_blocks": blocks
                }
            }),
        });
    }

    let finish_reason = if state.saw_tool_call {
        "tool_calls".to_owned()
    } else {
        match state.stop_reason.as_deref() {
            Some("tool_use") => "tool_calls".to_owned(),
            Some("end_turn" | "stop_sequence") | None => "stop".to_owned(),
            Some("max_tokens") => "length".to_owned(),
            Some(other) => other.to_owned(),
        }
    };
    sink(&ProviderEvent::Done { finish_reason });
    state.saw_done = true;
}

fn emit_anthropic_error<F>(payload: &Value, sink: &mut F) -> ExitCode
where
    F: FnMut(&ProviderEvent),
{
    let err_obj = payload.get("error").unwrap_or(payload);
    let code = err_obj
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| err_obj.get("code").and_then(Value::as_str))
        .unwrap_or("provider_error")
        .to_owned();
    let message = err_obj
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("upstream error event")
        .to_owned();
    let retryable = matches!(
        code.as_str(),
        "overloaded_error" | "rate_limit_error" | "api_error" | "timeout_error"
    );
    sink(&ProviderEvent::Error {
        code,
        message: message.clone(),
        retryable,
    });
    eprintln!("{TAG}: {message}");
    ExitCode::RuntimeError
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;

    use anix_proto::IR_VERSION;
    use anix_proto::ir::{HeaderTag, IrHeader, IrMeta, StderrMode, ToolCall};

    fn base_header(tools: Vec<anix_proto::ir::ToolDef>) -> IrHeader {
        IrHeader {
            ir_version: IR_VERSION,
            record_type: HeaderTag::Header,
            tools,
            max_output_tokens: None,
            params: BTreeMap::new(),
            meta: IrMeta {
                output_mode: None,
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn resolves_anthropic_endpoint_urls() {
        assert_eq!(
            endpoint_url("https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint_url("https://api.anthropic.com/v1"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint_url("https://proxy.local/anthropic/v1/messages"),
            "https://proxy.local/anthropic/v1/messages"
        );
    }

    #[test]
    fn coalesces_adjacent_user_and_tool_results_and_replays_signed_thinking() {
        let tmp_img =
            std::env::temp_dir().join(format!("anix-anthropic-test-{}.png", std::process::id()));
        fs::write(&tmp_img, b"\x89PNG\r\n\x1a\n").expect("write png");

        let mut header = base_header(vec![IrStream::shell_tool_def("shell")]);
        header.max_output_tokens = Some(4096);
        header
            .params
            .insert("reasoning_effort".into(), json!("low"));

        let stream = IrStream {
            header,
            messages: vec![
                IrMessage::system(0, "FROZEN_SYS"),
                IrMessage::user(1, "inspect"),
                IrMessage::Assistant {
                    turn_id: 2,
                    text: Some("checking".into()),
                    reasoning: Some("my thought".into()),
                    tool_calls: vec![
                        ToolCall {
                            id: "toolu_1".into(),
                            name: "msh".into(),
                            arguments: json!({"cmd": "aview a.png"}),
                        },
                        ToolCall {
                            id: "toolu_2".into(),
                            name: "msh".into(),
                            arguments: json!({"cmd": "false"}),
                        },
                    ],
                    provider_opaque: Some(json!({
                        "anthropic": {
                            "thinking_blocks": [
                                {"type": "thinking", "thinking": "my thought", "signature": "sig_abc"}
                            ]
                        }
                    })),
                    extra: BTreeMap::new(),
                },
                IrMessage::Tool {
                    turn_id: 3,
                    tool_call_id: "toolu_1".into(),
                    is_error: false,
                    text: "[t3] exit=0\n".into(),
                    media: vec![MediaAttachment {
                        mime: "image/png".into(),
                        path: tmp_img.display().to_string(),
                    }],
                    extra: BTreeMap::new(),
                },
                IrMessage::Tool {
                    turn_id: 4,
                    tool_call_id: "toolu_2".into(),
                    is_error: true,
                    text: "[t4] exit=1\n".into(),
                    media: vec![],
                    extra: BTreeMap::new(),
                },
                IrMessage::system(5, "<runtime_update>stderr=explicit</runtime_update>"),
            ],
        };

        let body = build_request(&stream, "claude-sonnet-4-5").expect("build anthropic request");
        let _ = fs::remove_file(&tmp_img);

        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(
            body["thinking"],
            json!({"type": "enabled", "budget_tokens": 2048})
        );
        assert_eq!(body["system"][0]["text"], "FROZEN_SYS");
        assert_eq!(
            body["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");

        // Strict alternation: user -> assistant -> single merged user (2 tool_results + runtime_update).
        let msgs = body["messages"].as_array().expect("messages");
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"][0]["type"], "thinking");
        assert_eq!(msgs[1]["content"][0]["signature"], "sig_abc");
        assert_eq!(msgs[1]["content"][1]["type"], "text");
        assert_eq!(msgs[1]["content"][2]["type"], "tool_use");

        assert_eq!(msgs[2]["role"], "user");
        let user_blocks = msgs[2]["content"].as_array().expect("merged blocks");
        assert_eq!(user_blocks.len(), 3);
        assert_eq!(user_blocks[0]["type"], "tool_result");
        assert_eq!(user_blocks[0]["content"][1]["type"], "image");
        assert_eq!(user_blocks[1]["is_error"], true);
        assert_eq!(user_blocks[2]["type"], "text");
        assert_eq!(
            user_blocks[2]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }

    #[test]
    fn sse_normalizes_inclusive_usage_captures_thinking_signature_and_tool_use() {
        let sse = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":200,\"cache_read_input_tokens\":3600,\"cache_creation_input_tokens\":400,\"output_tokens\":1}}}\n\n",
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"step 1\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig_xyz\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Running...\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":1}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_01\",\"name\":\"msh\",\"input\":{}}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"cmd\\\":\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":2,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"uname\\\"}\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":2}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"output_tokens\":380}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n"
        );

        let mut events = Vec::new();
        let exit = consume_sse_stream_with_sink(Cursor::new(sse.as_bytes()), &mut |ev| {
            events.push(ev.clone());
        });
        assert_eq!(exit, ExitCode::Success);
        assert_eq!(
            events,
            vec![
                ProviderEvent::ReasoningDelta {
                    text: "step 1".into()
                },
                ProviderEvent::TextDelta {
                    text: "Running...".into()
                },
                ProviderEvent::ToolCall {
                    id: "toolu_01".into(),
                    name: "msh".into(),
                    arguments: json!({"cmd": "uname"}),
                },
                // Inclusive input_tokens = 200 + 3600 + 400 = 4200!
                ProviderEvent::Usage {
                    input_tokens: 4200,
                    output_tokens: 380,
                    cached_tokens: Some(3600),
                    cache_write_tokens: Some(400),
                    reasoning_tokens: None,
                },
                ProviderEvent::Opaque {
                    data: json!({
                        "anthropic": {
                            "thinking_blocks": [
                                {"type": "thinking", "thinking": "step 1", "signature": "sig_xyz"}
                            ]
                        }
                    }),
                },
                ProviderEvent::Done {
                    finish_reason: "tool_calls".into()
                },
            ]
        );
    }

    #[test]
    fn sse_overloaded_error_is_retryable() {
        let sse = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
        let mut events = Vec::new();
        let exit = consume_sse_stream_with_sink(Cursor::new(sse.as_bytes()), &mut |ev| {
            events.push(ev.clone());
        });
        assert_eq!(exit, ExitCode::RuntimeError);
        assert_eq!(
            events,
            vec![ProviderEvent::Error {
                code: "overloaded_error".into(),
                message: "Overloaded".into(),
                retryable: true,
            }]
        );
    }
}
