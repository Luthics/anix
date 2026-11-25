//! OpenAI-compatible Chat Completions API (`/v1/chat/completions`) dialect (Sections 9.1 & 9.2).

use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader};

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, IrStream, MediaAttachment};
use anix_proto::provider::{ProviderEvent, TokenUsage};
use serde_json::{Map, Value, json};

use crate::common::{
    ProviderEnv, TranslateError, build_http_agent, drive_sse_reader, emit_event,
    parse_tool_arguments, read_media_base64, resolve_session_header, send_post_with_agent,
};

const TAG: &str = "anix-provider-openai-compat";

/// Namespace key stored inside `ProviderEvent::Opaque.data` / `IrMessage::Assistant.provider_opaque`.
pub const OPAQUE_NAMESPACE: &str = "openai_compat";

/// Resolve the Chat Completions endpoint URL from `base_url`.
pub fn endpoint_url(base_url: &str) -> String {
    if base_url.ends_with("/chat/completions") {
        base_url.to_owned()
    } else {
        format!("{base_url}/chat/completions")
    }
}

/// Build an OpenAI-compatible `/v1/chat/completions` JSON request payload from a Canonical IR v1 stream.
pub fn build_request(stream: &IrStream, model: &str) -> Result<Value, TranslateError> {
    build_request_with_session(stream, model, None)
}

/// Build an OpenAI-compatible `/v1/chat/completions` JSON request payload with optional
/// session routing affinity ID (v1.1.1 §9.2).
///
/// To avoid HTTP 400 errors on strict OpenAI-compatible endpoints, `session_id` is NOT
/// written into the JSON body by default; it is injected only when opted in via
/// `params.prompt_cache_key = true` or `params.session_id = true` (or explicit strings).
pub fn build_request_with_session(
    stream: &IrStream,
    model: &str,
    session_id: Option<&str>,
) -> Result<Value, TranslateError> {
    let mut messages: Vec<Value> = Vec::new();
    let mut saw_first_system = false;
    let mut pending_tool_media: Vec<(String, MediaAttachment)> = Vec::new();

    let flush_tool_media = |pending: &mut Vec<(String, MediaAttachment)>,
                            msgs: &mut Vec<Value>|
     -> Result<(), TranslateError> {
        if pending.is_empty() {
            return Ok(());
        }
        let call_ids: Vec<String> = pending.iter().map(|(id, _)| id.clone()).collect();
        let mut content = vec![json!({
            "type": "text",
            "text": format!("[attached visual media from tool call(s): {}]", call_ids.join(", "))
        })];
        for (_, m) in pending.drain(..) {
            content.push(encode_media_part(&m)?);
        }
        msgs.push(json!({
            "role": "user",
            "content": content
        }));
        Ok(())
    };

    for msg in &stream.messages {
        if !matches!(msg, IrMessage::Tool { .. }) {
            flush_tool_media(&mut pending_tool_media, &mut messages)?;
        }
        match msg {
            IrMessage::System { text, .. } => {
                if !saw_first_system {
                    messages.push(json!({
                        "role": "system",
                        "content": text
                    }));
                    saw_first_system = true;
                } else {
                    messages.push(json!({
                        "role": "user",
                        "content": text
                    }));
                }
            }
            IrMessage::User { text, media, .. } => {
                if media.is_empty() {
                    messages.push(json!({
                        "role": "user",
                        "content": text
                    }));
                } else {
                    let mut content = vec![json!({ "type": "text", "text": text })];
                    for m in media {
                        content.push(encode_media_part(m)?);
                    }
                    messages.push(json!({
                        "role": "user",
                        "content": content
                    }));
                }
            }
            IrMessage::Assistant {
                text,
                reasoning,
                tool_calls,
                provider_opaque,
                ..
            } => {
                let mut obj = Map::new();
                obj.insert("role".into(), Value::String("assistant".into()));

                match text.as_deref() {
                    Some(t) if !t.is_empty() => {
                        obj.insert("content".into(), Value::String(t.to_owned()));
                    }
                    _ => {
                        obj.insert("content".into(), Value::Null);
                    }
                }

                let compat_opaque = provider_opaque
                    .as_ref()
                    .and_then(|o| o.get(OPAQUE_NAMESPACE))
                    .and_then(Value::as_object);

                if let Some(opaque) = compat_opaque {
                    if let Some(field) = opaque.get("reasoning_field").and_then(Value::as_str)
                        && matches!(field, "reasoning_content" | "reasoning")
                    {
                        if let Some(r) = reasoning.as_deref()
                            && !r.is_empty()
                        {
                            obj.insert(field.into(), Value::String(r.to_owned()));
                        } else if !tool_calls.is_empty() {
                            obj.insert(field.into(), Value::String(String::new()));
                        }
                    }
                    if let Some(details) = opaque.get("reasoning_details") {
                        obj.insert("reasoning_details".into(), details.clone());
                    }
                }

                if !tool_calls.is_empty() {
                    let tc_extras = compat_opaque
                        .and_then(|o| o.get("tool_call_extra_content"))
                        .and_then(Value::as_object);
                    let calls: Vec<Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            let args_str = serde_json::to_string(&tc.arguments)
                                .unwrap_or_else(|_| "{}".to_owned());
                            let mut tc_obj = Map::new();
                            tc_obj.insert("id".into(), Value::String(tc.id.clone()));
                            tc_obj.insert("type".into(), Value::String("function".into()));
                            tc_obj.insert(
                                "function".into(),
                                json!({
                                    "name": tc.name,
                                    "arguments": args_str
                                }),
                            );
                            if let Some(extra) = tc_extras.and_then(|m| m.get(&tc.id)) {
                                tc_obj.insert("extra_content".into(), extra.clone());
                            }
                            Value::Object(tc_obj)
                        })
                        .collect();
                    obj.insert("tool_calls".into(), Value::Array(calls));
                }

                messages.push(Value::Object(obj));
            }
            IrMessage::Tool {
                tool_call_id,
                text,
                media,
                ..
            } => {
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": text
                }));
                for m in media {
                    pending_tool_media.push((tool_call_id.clone(), m.clone()));
                }
            }
        }
    }
    flush_tool_media(&mut pending_tool_media, &mut messages)?;

    let mut body = Map::new();
    body.insert("model".into(), Value::String(model.to_owned()));
    body.insert("messages".into(), Value::Array(messages));
    body.insert("stream".into(), Value::Bool(true));
    body.insert("stream_options".into(), json!({ "include_usage": true }));

    if !stream.header.tools.is_empty() {
        let tools: Vec<Value> = stream
            .header
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }

    if let Some(max_out) = stream.header.max_output_tokens
        && max_out >= 1
    {
        body.insert("max_tokens".into(), Value::from(max_out));
    }

    let trimmed_sid = session_id.map(str::trim).filter(|s| !s.is_empty());

    for (key, val) in &stream.header.params {
        match key.as_str() {
            // Handled at the HTTP header layer, not in the JSON request body.
            "session_header" => {}
            "prompt_cache_key" | "session_id" => match val {
                Value::Bool(true) => {
                    if let Some(sid) = trimmed_sid {
                        body.insert(key.clone(), Value::String(sid.to_owned()));
                    }
                }
                Value::Bool(false) | Value::Null => {
                    body.remove(key);
                }
                Value::String(s) if !s.trim().is_empty() => {
                    body.insert(key.clone(), Value::String(s.clone()));
                }
                _ => {
                    eprintln!("{TAG}: warning: ignoring invalid `{key}` param");
                }
            },
            "stream_options" => {
                if val.is_null() || *val == Value::Bool(false) {
                    body.remove("stream_options");
                } else if val.is_object() {
                    body.insert("stream_options".into(), val.clone());
                } else {
                    eprintln!("{TAG}: warning: ignoring invalid `stream_options` param");
                }
            }
            "max_completion_tokens" => {
                body.remove("max_tokens");
                body.insert("max_completion_tokens".into(), val.clone());
            }
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
            "reasoning_effort"
            | "temperature"
            | "top_p"
            | "frequency_penalty"
            | "presence_penalty"
            | "stop"
            | "seed"
            | "logit_bias"
            | "logprobs"
            | "top_logprobs"
            | "response_format"
            | "tool_choice"
            | "parallel_tool_calls"
            | "service_tier"
            | "store"
            | "metadata"
            | "user"
            | "thinking"
            | "enable_thinking"
            | "reasoning"
            | "chat_template_kwargs"
            | "top_k"
            | "min_p"
            | "repetition_penalty" => {
                body.insert(key.clone(), val.clone());
            }
            other => {
                eprintln!("{TAG}: warning: ignoring unsupported param `{other}`");
            }
        }
    }

    Ok(Value::Object(body))
}

fn encode_media_part(media: &MediaAttachment) -> Result<Value, TranslateError> {
    let b64 = read_media_base64(media)?;
    let data_uri = format!("data:{};base64,{b64}", media.mime);
    Ok(json!({
        "type": "image_url",
        "image_url": {
            "url": data_uri
        }
    }))
}

#[derive(Debug, Default)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    extra_content: Option<Value>,
}

#[derive(Debug, Default)]
struct StreamState {
    pending_calls: BTreeMap<usize, PendingToolCall>,
    emitted_call_ids: HashSet<String>,
    next_fallback_index: usize,
    reasoning_field: Option<&'static str>,
    reasoning_details: Vec<Value>,
    tool_call_extra_content: Map<String, Value>,
    last_usage: Option<TokenUsage>,
    finish_reason: Option<String>,
    saw_tool_call: bool,
    saw_done: bool,
}

/// Execute a single-shot streaming POST to `{base_url}/chat/completions`.
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
    let mut headers: Vec<(String, String)> = Vec::new();
    if !env.api_key.trim().is_empty() {
        headers.push(("Authorization".into(), format!("Bearer {}", env.api_key)));
    }
    if let Some(h) = resolve_session_header(&stream.header.params, env.session_id.as_deref(), TAG) {
        headers.push(h);
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

/// Parse a non-streaming `application/json` Chat Completion (or 200-wrapped error) body.
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

    if let Some(exit) = maybe_handle_inline_error(&payload, sink) {
        return exit;
    }

    let mut state = StreamState::default();
    if let Some(usage) = payload.get("usage").and_then(parse_usage) {
        state.last_usage = Some(usage);
    }

    if let Some(choice) = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    {
        if let Some(msg) = choice.get("message") {
            apply_message_or_delta(msg, &mut state, sink);
        }
        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str)
            && !fr.is_empty()
        {
            state.finish_reason = Some(fr.to_owned());
        }
    }

    finalize_stream(&mut state, sink);
    ExitCode::Success
}

/// Consume an OpenAI-compatible Chat Completions SSE stream into canonical [`ProviderEvent`]s.
pub fn consume_sse_stream_with_sink<R, F>(reader: R, sink: &mut F) -> ExitCode
where
    R: BufRead,
    F: FnMut(&ProviderEvent),
{
    let mut state = StreamState::default();
    match drive_sse_reader(TAG, reader, sink, |_ev, data, s| {
        dispatch_sse_frame(data, &mut state, s)
    }) {
        Ok(Some(code)) => code,
        Ok(None) => {
            if state.saw_done {
                ExitCode::Success
            } else if state.finish_reason.is_some() {
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

fn dispatch_sse_frame<F>(data: &str, state: &mut StreamState, sink: &mut F) -> Option<ExitCode>
where
    F: FnMut(&ProviderEvent),
{
    if data.trim() == "[DONE]" {
        if !state.saw_done {
            finalize_stream(state, sink);
        }
        return Some(ExitCode::Success);
    }

    let Ok(payload) = serde_json::from_str::<Value>(data) else {
        return None;
    };

    if let Some(exit) = maybe_handle_inline_error(&payload, sink) {
        return Some(exit);
    }

    if let Some(usage) = payload.get("usage").and_then(parse_usage) {
        state.last_usage = Some(usage);
    }

    if let Some(choice) = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    {
        if let Some(delta) = choice.get("delta") {
            apply_message_or_delta(delta, state, sink);
        } else if let Some(msg) = choice.get("message") {
            apply_message_or_delta(msg, state, sink);
        }

        if let Some(fr) = choice.get("finish_reason").and_then(Value::as_str)
            && !fr.is_empty()
        {
            state.finish_reason = Some(fr.to_owned());
        }
    }

    None
}

fn apply_message_or_delta<F>(obj: &Value, state: &mut StreamState, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    let mut emitted_reasoning = false;
    if let Some(r) = obj.get("reasoning_content").and_then(Value::as_str) {
        state.reasoning_field = Some("reasoning_content");
        if !r.is_empty() {
            sink(&ProviderEvent::ReasoningDelta { text: r.to_owned() });
            emitted_reasoning = true;
        }
    } else if let Some(r) = obj.get("reasoning").and_then(Value::as_str) {
        if state.reasoning_field.is_none() {
            state.reasoning_field = Some("reasoning");
        }
        if !r.is_empty() {
            sink(&ProviderEvent::ReasoningDelta { text: r.to_owned() });
            emitted_reasoning = true;
        }
    }

    if let Some(details) = obj.get("reasoning_details").and_then(Value::as_array) {
        for item in details {
            push_or_merge_reasoning_detail(&mut state.reasoning_details, item);
            if !emitted_reasoning
                && let Some(text) = item
                    .get("text")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("summary").and_then(Value::as_str))
                && !text.is_empty()
            {
                sink(&ProviderEvent::ReasoningDelta {
                    text: text.to_owned(),
                });
            }
        }
    }

    if let Some(content) = obj.get("content").and_then(Value::as_str)
        && !content.is_empty()
    {
        sink(&ProviderEvent::TextDelta {
            text: content.to_owned(),
        });
    }
    if let Some(refusal) = obj.get("refusal").and_then(Value::as_str)
        && !refusal.is_empty()
    {
        sink(&ProviderEvent::TextDelta {
            text: refusal.to_owned(),
        });
    }

    if let Some(tool_calls) = obj.get("tool_calls").and_then(Value::as_array) {
        for tc in tool_calls {
            accumulate_tool_call_delta(tc, state);
        }
    }
}

fn accumulate_tool_call_delta(tc: &Value, state: &mut StreamState) {
    let chunk_id = tc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());

    let idx = if let Some(explicit_idx) = tc.get("index").and_then(Value::as_u64) {
        let explicit_idx = explicit_idx as usize;
        if let Some(new_id) = chunk_id
            && let Some(existing) = state.pending_calls.get(&explicit_idx)
            && !existing.id.is_empty()
            && existing.id != new_id
        {
            let next = state
                .pending_calls
                .keys()
                .next_back()
                .map_or(0, |k| k + 1)
                .max(state.next_fallback_index);
            state.next_fallback_index = next + 1;
            next
        } else {
            state.next_fallback_index = state.next_fallback_index.max(explicit_idx + 1);
            explicit_idx
        }
    } else if let Some(new_id) = chunk_id {
        if let Some((&existing_idx, _)) = state.pending_calls.iter().find(|(_, p)| p.id == new_id) {
            existing_idx
        } else {
            let next = state.next_fallback_index;
            state.next_fallback_index = next + 1;
            next
        }
    } else if let Some((&last_idx, _)) = state.pending_calls.iter().next_back() {
        last_idx
    } else {
        let next = state.next_fallback_index;
        state.next_fallback_index = next + 1;
        next
    };

    let entry = state.pending_calls.entry(idx).or_default();
    if let Some(id) = chunk_id {
        entry.id = id.to_owned();
    }
    if let Some(func) = tc.get("function") {
        if let Some(name) = func.get("name").and_then(Value::as_str)
            && !name.is_empty()
            && entry.name.is_empty()
        {
            entry.name = name.to_owned();
        }
        if let Some(args_val) = func.get("arguments") {
            match args_val {
                Value::String(s) => entry.arguments.push_str(s),
                Value::Object(_) => {
                    if let Ok(serialized) = serde_json::to_string(args_val) {
                        entry.arguments = serialized;
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(extra) = tc.get("extra_content")
        && !extra.is_null()
    {
        entry.extra_content = Some(extra.clone());
    }
}

fn finalize_stream<F>(state: &mut StreamState, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    let pending = std::mem::take(&mut state.pending_calls);
    for (idx, tc) in pending {
        if tc.name.is_empty() {
            continue;
        }
        let call_id = if tc.id.is_empty() {
            format!("call_{idx}")
        } else {
            tc.id
        };
        if state.emitted_call_ids.insert(call_id.clone()) {
            if let Some(extra) = tc.extra_content {
                state.tool_call_extra_content.insert(call_id.clone(), extra);
            }
            sink(&ProviderEvent::ToolCall {
                id: call_id,
                name: tc.name,
                arguments: parse_tool_arguments(&tc.arguments),
            });
            state.saw_tool_call = true;
        }
    }

    if let Some(u) = state.last_usage.take() {
        sink(&ProviderEvent::Usage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
            cached_tokens: u.cached_tokens,
            cache_write_tokens: u.cache_write_tokens,
            reasoning_tokens: u.reasoning_tokens,
        });
    }

    let mut compat_meta = Map::new();
    if let Some(field) = state.reasoning_field {
        compat_meta.insert("reasoning_field".into(), Value::String(field.into()));
    }
    if !state.reasoning_details.is_empty() {
        let details = std::mem::take(&mut state.reasoning_details);
        compat_meta.insert("reasoning_details".into(), Value::Array(details));
    }
    if !state.tool_call_extra_content.is_empty() {
        let extras = std::mem::take(&mut state.tool_call_extra_content);
        compat_meta.insert("tool_call_extra_content".into(), Value::Object(extras));
    }
    if !compat_meta.is_empty() {
        sink(&ProviderEvent::Opaque {
            data: json!({ OPAQUE_NAMESPACE: Value::Object(compat_meta) }),
        });
    }

    let finish_reason = if state.saw_tool_call {
        "tool_calls".to_owned()
    } else {
        state
            .finish_reason
            .clone()
            .unwrap_or_else(|| "stop".to_owned())
    };
    sink(&ProviderEvent::Done { finish_reason });
    state.saw_done = true;
}

fn maybe_handle_inline_error<F>(payload: &Value, sink: &mut F) -> Option<ExitCode>
where
    F: FnMut(&ProviderEvent),
{
    let err_obj = payload.get("error").filter(|v| !v.is_null())?;
    let code = err_obj
        .get("code")
        .and_then(|v| {
            v.as_str()
                .map(ToOwned::to_owned)
                .or_else(|| v.as_u64().map(|n| n.to_string()))
        })
        .or_else(|| {
            err_obj
                .get("type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "provider_error".into());
    let message = err_obj
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| err_obj.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "upstream error event".into());
    let numeric_code = err_obj
        .get("code")
        .and_then(Value::as_u64)
        .or_else(|| code.parse::<u64>().ok());
    let retryable = matches!(
        code.as_str(),
        "rate_limit_exceeded"
            | "rate_limit"
            | "server_error"
            | "overloaded"
            | "overloaded_error"
            | "upstream_error"
            | "timeout"
    ) || numeric_code.is_some_and(|n| n == 429 || n == 408 || n >= 500);

    sink(&ProviderEvent::Error {
        code,
        message: message.clone(),
        retryable,
    });
    eprintln!("{TAG}: {message}");
    Some(ExitCode::RuntimeError)
}

fn parse_usage(usage: &Value) -> Option<TokenUsage> {
    if !usage.is_object() {
        return None;
    }
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("input_tokens").and_then(Value::as_u64))?;
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .or_else(|| usage.get("output_tokens").and_then(Value::as_u64))
        .unwrap_or(0);

    let cached_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| usage.get("prompt_cache_hit_tokens").and_then(Value::as_u64))
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(Value::as_u64)
        })
        .or_else(|| usage.get("cache_read_input_tokens").and_then(Value::as_u64));

    let cache_write_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|d| {
            d.get("cache_write_tokens")
                .or_else(|| d.get("cache_creation_tokens"))
        })
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .get("cache_creation_input_tokens")
                .and_then(Value::as_u64)
        })
        .or_else(|| {
            usage
                .get("input_tokens_details")
                .and_then(|d| d.get("cache_write_tokens"))
                .and_then(Value::as_u64)
        });

    let reasoning_tokens = usage
        .get("completion_tokens_details")
        .and_then(|d| d.get("reasoning_tokens"))
        .and_then(Value::as_u64)
        .or_else(|| {
            usage
                .get("output_tokens_details")
                .and_then(|d| d.get("reasoning_tokens"))
                .and_then(Value::as_u64)
        });

    Some(TokenUsage {
        input_tokens,
        output_tokens,
        cached_tokens,
        cache_write_tokens,
        reasoning_tokens,
    })
}

fn push_or_merge_reasoning_detail(acc: &mut Vec<Value>, item: &Value) {
    if let Some(item_obj) = item.as_object()
        && item_obj.get("data").is_none()
        && item_obj.get("signature").is_none()
        && let Some(last) = acc.last_mut().and_then(Value::as_object_mut)
        && last.get("type") == item_obj.get("type")
        && last.get("index") == item_obj.get("index")
        && last.get("id") == item_obj.get("id")
        && last.get("data").is_none()
        && last.get("signature").is_none()
    {
        if let Some(delta) = item_obj.get("text").and_then(Value::as_str)
            && let Some(Value::String(existing)) = last.get_mut("text")
        {
            existing.push_str(delta);
            return;
        }
        if let Some(delta) = item_obj.get("summary").and_then(Value::as_str)
            && let Some(Value::String(existing)) = last.get_mut("summary")
        {
            existing.push_str(delta);
            return;
        }
    }
    acc.push(item.clone());
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
                stderr_mode: StderrMode::Auto,
                cwd: "/tmp".into(),
                turn_budget: None,
                extra: BTreeMap::new(),
            },
            extra: BTreeMap::new(),
        }
    }

    fn collect_sse(sse: &str) -> (ExitCode, Vec<ProviderEvent>) {
        let mut events = Vec::new();
        let exit = consume_sse_stream_with_sink(Cursor::new(sse.as_bytes()), &mut |ev| {
            events.push(ev.clone());
        });
        (exit, events)
    }

    #[test]
    fn endpoint_url_appends_chat_completions_once() {
        assert_eq!(
            endpoint_url("https://gateway.example.com/v1"),
            "https://gateway.example.com/v1/chat/completions"
        );
        assert_eq!(
            endpoint_url("https://gateway.example.com/v1/chat/completions"),
            "https://gateway.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn translates_frozen_system_degraded_runtime_update_tools_and_params() {
        let mut header = base_header(vec![IrStream::msh_tool_def()]);
        header.max_output_tokens = Some(4096);
        header
            .params
            .insert("reasoning_effort".into(), json!("high"));
        header.params.insert("temperature".into(), json!(0.2));
        header.params.insert(
            "extra_body".into(),
            json!({"custom_flag": true, "stream": false}),
        );
        header
            .params
            .insert("unknown_key_ignored".into(), json!("x"));

        let stream = IrStream {
            header,
            messages: vec![
                IrMessage::system(0, "FROZEN_SYSTEM_PROMPT"),
                IrMessage::user(1, "hello"),
                IrMessage::system(2, "<runtime_update>stderr=explicit</runtime_update>"),
            ],
        };

        let body = build_request(&stream, "deepseek-v4.1-flash").expect("translate request");
        assert_eq!(body["model"], "deepseek-v4.1-flash");
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"], json!({"include_usage": true}));
        assert_eq!(body["max_tokens"], 4096);
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["temperature"], 0.2);
        assert_eq!(body["custom_flag"], true);
        assert!(body.get("unknown_key_ignored").is_none());
        assert_eq!(
            body["messages"],
            json!([
                {"role": "system", "content": "FROZEN_SYSTEM_PROMPT"},
                {"role": "user", "content": "hello"},
                {"role": "user", "content": "<runtime_update>stderr=explicit</runtime_update>"}
            ])
        );
    }

    #[test]
    fn omits_tools_when_empty_and_supports_disabling_stream_options() {
        let mut header = base_header(vec![]);
        header.params.insert("stream_options".into(), json!(false));
        header.max_output_tokens = Some(2048);
        header
            .params
            .insert("max_completion_tokens".into(), json!(1024));

        let stream = IrStream {
            header,
            messages: vec![IrMessage::user(1, "summarize")],
        };

        let body = build_request(&stream, "gpt-5").expect("translate");
        assert!(body.get("tools").is_none());
        assert!(body.get("stream_options").is_none());
        assert!(body.get("max_tokens").is_none());
        assert_eq!(body["max_completion_tokens"], 1024);
    }

    #[test]
    fn replays_opaque_reasoning_and_flushes_tool_media_after_batch() {
        let tmp_img =
            std::env::temp_dir().join(format!("anix-compat-test-{}.png", std::process::id()));
        fs::write(&tmp_img, b"\x89PNG\r\n\x1a\n").expect("write tmp png");

        let stream = IrStream {
            header: base_header(vec![IrStream::msh_tool_def()]),
            messages: vec![
                IrMessage::user(1, "inspect"),
                IrMessage::Assistant {
                    turn_id: 2,
                    text: None,
                    reasoning: Some("plan step".into()),
                    tool_calls: vec![
                        ToolCall {
                            id: "call_1".into(),
                            name: "msh".into(),
                            arguments: json!({"cmd": "aview a.png"}),
                        },
                        ToolCall {
                            id: "call_2".into(),
                            name: "msh".into(),
                            arguments: json!({"cmd": "ls"}),
                        },
                    ],
                    provider_opaque: Some(json!({
                        "openai_compat": {
                            "reasoning_field": "reasoning_content",
                            "tool_call_extra_content": {
                                "call_1": { "google": { "thought_signature": "sig" } }
                            }
                        }
                    })),
                    extra: BTreeMap::new(),
                },
                IrMessage::Tool {
                    turn_id: 3,
                    tool_call_id: "call_1".into(),
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
                    tool_call_id: "call_2".into(),
                    is_error: false,
                    text: "[t4] exit=0\nfile.txt".into(),
                    media: vec![],
                    extra: BTreeMap::new(),
                },
            ],
        };

        let body = build_request(&stream, "deepseek-v4.1-flash").expect("translate");
        let _ = fs::remove_file(&tmp_img);

        let msgs = body["messages"].as_array().expect("messages");
        assert_eq!(msgs.len(), 5);
        assert_eq!(msgs[1]["reasoning_content"], "plan step");
        assert_eq!(
            msgs[1]["tool_calls"][0]["extra_content"],
            json!({"google": {"thought_signature": "sig"}})
        );
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[3]["role"], "tool");
        assert_eq!(msgs[4]["role"], "user");
    }

    #[test]
    fn sse_streams_reasoning_text_and_trailing_usage_after_finish_reason() {
        let sse = concat!(
            ": keep-alive\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"think \"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"reasoning_content\":\"hard\",\"reasoning\":\"dup\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello \"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"world\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":null}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":120,\"completion_tokens\":30,\"prompt_tokens_details\":{\"cached_tokens\":80},\"completion_tokens_details\":{\"reasoning_tokens\":12}}}\n\n",
            "data: [DONE]\n\n"
        );

        let (exit, events) = collect_sse(sse);
        assert_eq!(exit, ExitCode::Success);
        assert_eq!(
            events,
            vec![
                ProviderEvent::ReasoningDelta {
                    text: "think ".into()
                },
                ProviderEvent::ReasoningDelta {
                    text: "hard".into()
                },
                ProviderEvent::TextDelta {
                    text: "Hello ".into()
                },
                ProviderEvent::TextDelta {
                    text: "world".into()
                },
                ProviderEvent::Usage {
                    input_tokens: 120,
                    output_tokens: 30,
                    cached_tokens: Some(80),
                    cache_write_tokens: None,
                    reasoning_tokens: Some(12),
                },
                ProviderEvent::Opaque {
                    data: json!({"openai_compat": {"reasoning_field": "reasoning_content"}}),
                },
                ProviderEvent::Done {
                    finish_reason: "stop".into()
                },
            ]
        );
    }

    #[test]
    fn sse_accumulates_multi_chunk_tool_calls_and_extra_content() {
        let sse = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_a\",\"type\":\"function\",\"extra_content\":{\"google\":{\"thought_signature\":\"sig1\"}},\"function\":{\"name\":\"msh\",\"arguments\":\"{\\\"cmd\\\": \"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_b\",\"type\":\"function\",\"function\":{\"name\":\"msh\",\"arguments\":\"{\\\"cmd\\\":\\\"pwd\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"ls\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":50,\"completion_tokens\":15,\"prompt_cache_hit_tokens\":40}}\n\n",
            "data: [DONE]\n\n"
        );

        let (exit, events) = collect_sse(sse);
        assert_eq!(exit, ExitCode::Success);
        assert_eq!(
            events,
            vec![
                ProviderEvent::ToolCall {
                    id: "call_a".into(),
                    name: "msh".into(),
                    arguments: json!({"cmd": "ls"}),
                },
                ProviderEvent::ToolCall {
                    id: "call_b".into(),
                    name: "msh".into(),
                    arguments: json!({"cmd": "pwd"}),
                },
                ProviderEvent::Usage {
                    input_tokens: 50,
                    output_tokens: 15,
                    cached_tokens: Some(40),
                    cache_write_tokens: None,
                    reasoning_tokens: None,
                },
                ProviderEvent::Opaque {
                    data: json!({
                        "openai_compat": {
                            "tool_call_extra_content": {
                                "call_a": { "google": { "thought_signature": "sig1" } }
                            }
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
    fn sse_clean_eof_after_finish_reason_without_done_and_truncated_error() {
        let sse_ok = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"length\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5}}\n\n"
        );
        let (exit, events) = collect_sse(sse_ok);
        assert_eq!(exit, ExitCode::Success);
        assert!(matches!(
            events.last(),
            Some(ProviderEvent::Done { finish_reason }) if finish_reason == "length"
        ));

        let sse_trunc = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"half\"}}]}\n\n";
        let (exit2, events2) = collect_sse(sse_trunc);
        assert_eq!(exit2, ExitCode::RuntimeError);
        assert!(matches!(
            events2.last(),
            Some(ProviderEvent::Error { code, retryable: true, .. }) if code == "stream_truncated"
        ));
    }

    #[test]
    fn non_streaming_json_fallback_parses_message_and_inline_error() {
        let raw = r#"{
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "reasoning_content": "step1", "content": "answer" },
                "finish_reason": "stop"
            }],
            "usage": { "prompt_tokens": 20, "completion_tokens": 8 }
        }"#;
        let mut events = Vec::new();
        let exit = consume_json_response(raw, &mut |ev| events.push(ev.clone()));
        assert_eq!(exit, ExitCode::Success);
        assert!(events.contains(&ProviderEvent::TextDelta {
            text: "answer".into()
        }));

        let err_raw = r#"{"error":{"code":"rate_limit_exceeded","message":"slow down"}}"#;
        let mut err_events = Vec::new();
        let err_exit = consume_json_response(err_raw, &mut |ev| err_events.push(ev.clone()));
        assert_eq!(err_exit, ExitCode::RuntimeError);
        assert_eq!(
            err_events,
            vec![ProviderEvent::Error {
                code: "rate_limit_exceeded".into(),
                message: "slow down".into(),
                retryable: true,
            }]
        );
    }
}
