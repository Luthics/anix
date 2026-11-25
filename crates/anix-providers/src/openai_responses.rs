//! OpenAI Responses API (`/v1/responses`) dialect (Sections 9.1 & 9.2).

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrMessage, IrStream, MediaAttachment};
use anix_proto::provider::ProviderEvent;
use serde_json::{Map, Value, json};

use crate::common::{
    ProviderEnv, TranslateError, build_http_agent, drive_sse_reader, emit_event,
    parse_tool_arguments, read_media_base64, resolve_session_header, send_post_with_agent,
};

const TAG: &str = "anix-provider-openai-responses";

/// Resolve the Responses API endpoint URL from `base_url`.
pub fn endpoint_url(base_url: &str) -> String {
    if base_url.ends_with("/responses") {
        base_url.to_owned()
    } else {
        format!("{base_url}/responses")
    }
}

/// Build an OpenAI `/v1/responses` JSON request payload from a Canonical IR v1 stream.
pub fn build_request(stream: &IrStream, model: &str) -> Result<Value, TranslateError> {
    build_request_with_session(stream, model, None)
}

/// Build an OpenAI `/v1/responses` JSON request payload with optional session routing affinity ID
/// (v1.1.1 §9.2). Defaults `prompt_cache_key` to `session_id` unless overridden or disabled via
/// `params.prompt_cache_key = false | null`.
pub fn build_request_with_session(
    stream: &IrStream,
    model: &str,
    session_id: Option<&str>,
) -> Result<Value, TranslateError> {
    let mut instructions: Option<String> = None;
    let mut input_items: Vec<Value> = Vec::new();
    let mut pending_tool_media: Vec<(String, MediaAttachment)> = Vec::new();

    let flush_tool_media = |pending: &mut Vec<(String, MediaAttachment)>,
                            items: &mut Vec<Value>|
     -> Result<(), TranslateError> {
        if pending.is_empty() {
            return Ok(());
        }
        let call_ids: Vec<String> = pending.iter().map(|(id, _)| id.clone()).collect();
        let mut content = vec![json!({
            "type": "input_text",
            "text": format!("[attached visual media from tool call(s): {}]", call_ids.join(", "))
        })];
        for (_, m) in pending.drain(..) {
            content.push(encode_media_part(&m)?);
        }
        items.push(json!({
            "type": "message",
            "role": "user",
            "content": content
        }));
        Ok(())
    };

    for msg in &stream.messages {
        if !matches!(msg, IrMessage::Tool { .. }) {
            flush_tool_media(&mut pending_tool_media, &mut input_items)?;
        }
        match msg {
            IrMessage::System { text, .. } => {
                if instructions.is_none() {
                    // Frozen first system prefix: pass verbatim into `instructions`.
                    instructions = Some(text.clone());
                } else {
                    // Trailing runtime_update system message: degrade to `user` item for
                    // broad compatibility across OpenAI Responses and compatible gateways.
                    input_items.push(json!({
                        "type": "message",
                        "role": "user",
                        "content": [{ "type": "input_text", "text": text }]
                    }));
                }
            }
            IrMessage::User { text, media, .. } => {
                let mut content = vec![json!({ "type": "input_text", "text": text })];
                for m in media {
                    content.push(encode_media_part(m)?);
                }
                input_items.push(json!({
                    "type": "message",
                    "role": "user",
                    "content": content
                }));
            }
            IrMessage::Assistant {
                text,
                tool_calls,
                provider_opaque,
                ..
            } => {
                if let Some(opaque) = provider_opaque
                    && let Some(items) = opaque
                        .get("responses_reasoning_items")
                        .and_then(Value::as_array)
                {
                    for item in items {
                        input_items.push(item.clone());
                    }
                }
                if let Some(t) = text
                    && !t.is_empty()
                {
                    input_items.push(json!({
                        "type": "message",
                        "role": "assistant",
                        "content": [{ "type": "output_text", "text": t }]
                    }));
                }
                for tc in tool_calls {
                    let args_str =
                        serde_json::to_string(&tc.arguments).unwrap_or_else(|_| "{}".to_owned());
                    input_items.push(json!({
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.name,
                        "arguments": args_str
                    }));
                }
            }
            IrMessage::Tool {
                tool_call_id,
                text,
                media,
                ..
            } => {
                input_items.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_call_id,
                    "output": text
                }));
                for m in media {
                    pending_tool_media.push((tool_call_id.clone(), m.clone()));
                }
            }
        }
    }
    flush_tool_media(&mut pending_tool_media, &mut input_items)?;

    let mut body = Map::new();
    body.insert("model".into(), Value::String(model.to_owned()));
    body.insert("stream".into(), Value::Bool(true));
    if let Some(inst) = instructions {
        body.insert("instructions".into(), Value::String(inst));
    }
    body.insert("input".into(), Value::Array(input_items));

    if !stream.header.tools.is_empty() {
        let tools: Vec<Value> = stream
            .header
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters
                })
            })
            .collect();
        body.insert("tools".into(), Value::Array(tools));
    }

    if let Some(max_out) = stream.header.max_output_tokens
        && max_out >= 1
    {
        body.insert("max_output_tokens".into(), Value::from(max_out));
    }

    let trimmed_sid = session_id.map(str::trim).filter(|s| !s.is_empty());
    let mut prompt_cache_key_configured = false;

    for (key, val) in &stream.header.params {
        match key.as_str() {
            // Handled at the HTTP header layer, not in the JSON request body.
            "session_header" => {}
            "prompt_cache_key" => {
                prompt_cache_key_configured = true;
                match val {
                    Value::Bool(false) | Value::Null => {
                        body.remove("prompt_cache_key");
                    }
                    Value::Bool(true) => {
                        if let Some(sid) = trimmed_sid {
                            body.insert("prompt_cache_key".into(), Value::String(sid.to_owned()));
                        }
                    }
                    Value::String(s) if !s.trim().is_empty() => {
                        body.insert("prompt_cache_key".into(), Value::String(s.clone()));
                    }
                    _ => {
                        eprintln!("{TAG}: warning: ignoring invalid `prompt_cache_key` param");
                    }
                }
            }
            "reasoning_effort" => {
                let reasoning = body
                    .entry("reasoning")
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(obj) = reasoning.as_object_mut() {
                    obj.insert("effort".into(), val.clone());
                } else {
                    body.insert("reasoning".into(), json!({ "effort": val }));
                }
            }
            "temperature"
            | "top_p"
            | "max_tool_calls"
            | "parallel_tool_calls"
            | "truncation"
            | "service_tier"
            | "store"
            | "metadata"
            | "text"
            | "reasoning"
            | "include"
            | "user" => {
                body.insert(key.clone(), val.clone());
            }
            other => {
                eprintln!("{TAG}: warning: ignoring unsupported param `{other}`");
            }
        }
    }

    if !prompt_cache_key_configured && let Some(sid) = trimmed_sid {
        body.insert("prompt_cache_key".into(), Value::String(sid.to_owned()));
    }

    Ok(Value::Object(body))
}

fn encode_media_part(media: &MediaAttachment) -> Result<Value, TranslateError> {
    let b64 = read_media_base64(media)?;
    let data_uri = format!("data:{};base64,{b64}", media.mime);
    Ok(json!({
        "type": "input_image",
        "image_url": data_uri
    }))
}

#[derive(Debug, Default)]
struct PendingFunctionCall {
    call_id: String,
    name: String,
    arguments: String,
}

#[derive(Debug, Default)]
struct ResponsesStreamState {
    pending_calls: HashMap<String, PendingFunctionCall>,
    emitted_call_ids: HashSet<String>,
    opaque_items: Vec<Value>,
    saw_tool_call: bool,
    saw_done: bool,
}

/// Execute a single-shot streaming POST to `{base_url}/responses`.
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

    let reader = BufReader::new(response.into_body().into_reader());
    consume_sse_stream_with_sink(reader, &mut sink)
}

/// Consume an OpenAI Responses SSE stream into canonical [`ProviderEvent`]s.
pub fn consume_sse_stream_with_sink<R, F>(reader: R, sink: &mut F) -> ExitCode
where
    R: BufRead,
    F: FnMut(&ProviderEvent),
{
    let mut state = ResponsesStreamState::default();
    match drive_sse_reader(TAG, reader, sink, |ev, data, s| {
        dispatch_sse_frame(ev, data, &mut state, s)
    }) {
        Ok(Some(code)) => code,
        Ok(None) => {
            if state.saw_done {
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
    state: &mut ResponsesStreamState,
    sink: &mut F,
) -> Option<ExitCode>
where
    F: FnMut(&ProviderEvent),
{
    if data.trim() == "[DONE]" {
        if !state.saw_done {
            flush_opaque(&mut state.opaque_items, sink);
            let finish_reason = if state.saw_tool_call {
                "tool_calls"
            } else {
                "stop"
            };
            sink(&ProviderEvent::Done {
                finish_reason: finish_reason.into(),
            });
            state.saw_done = true;
        }
        return Some(ExitCode::Success);
    }

    let Ok(payload) = serde_json::from_str::<Value>(data) else {
        return None;
    };

    let ev_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or(sse_event);

    match ev_type {
        "response.output_text.delta" => {
            if let Some(delta) = payload.get("delta").and_then(Value::as_str)
                && !delta.is_empty()
            {
                sink(&ProviderEvent::TextDelta {
                    text: delta.to_owned(),
                });
            }
        }
        "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
            if let Some(delta) = payload.get("delta").and_then(Value::as_str)
                && !delta.is_empty()
            {
                sink(&ProviderEvent::ReasoningDelta {
                    text: delta.to_owned(),
                });
            }
        }
        "response.output_item.added" => {
            if let Some(item) = payload.get("item")
                && item.get("type").and_then(Value::as_str) == Some("function_call")
            {
                let item_id = item
                    .get("id")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("call_id").and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_owned();
                let call_id = item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .unwrap_or(&item_id)
                    .to_owned();
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let arguments = item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if !item_id.is_empty() {
                    state.pending_calls.insert(
                        item_id,
                        PendingFunctionCall {
                            call_id,
                            name,
                            arguments,
                        },
                    );
                }
            }
        }
        "response.function_call_arguments.delta" => {
            if let Some(item_id) = payload.get("item_id").and_then(Value::as_str)
                && let Some(delta) = payload.get("delta").and_then(Value::as_str)
                && let Some(entry) = state.pending_calls.get_mut(item_id)
            {
                entry.arguments.push_str(delta);
            }
        }
        "response.function_call_arguments.done" => {
            if let Some(item_id) = payload.get("item_id").and_then(Value::as_str)
                && let Some(entry) = state.pending_calls.get_mut(item_id)
            {
                if let Some(args) = payload.get("arguments").and_then(Value::as_str) {
                    entry.arguments = args.to_owned();
                }
                if !entry.call_id.is_empty()
                    && !entry.name.is_empty()
                    && state.emitted_call_ids.insert(entry.call_id.clone())
                {
                    sink(&ProviderEvent::ToolCall {
                        id: entry.call_id.clone(),
                        name: entry.name.clone(),
                        arguments: parse_tool_arguments(&entry.arguments),
                    });
                    state.saw_tool_call = true;
                }
            }
        }
        "response.output_item.done" => {
            if let Some(item) = payload.get("item") {
                match item.get("type").and_then(Value::as_str) {
                    Some("function_call") => {
                        emit_function_call_item(item, state, sink);
                    }
                    Some("reasoning") if item.get("encrypted_content").is_some() => {
                        state.opaque_items.push(item.clone());
                    }
                    _ => {}
                }
            }
        }
        "response.completed" | "response.incomplete" => {
            if let Some(resp) = payload.get("response") {
                if let Some(outputs) = resp.get("output").and_then(Value::as_array) {
                    for item in outputs {
                        if item.get("type").and_then(Value::as_str) == Some("function_call") {
                            emit_function_call_item(item, state, sink);
                        }
                    }
                }

                if let Some(usage) = resp.get("usage") {
                    let input_tokens = usage
                        .get("input_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let output_tokens = usage
                        .get("output_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0);
                    let cached_tokens = usage
                        .get("input_tokens_details")
                        .and_then(|d| d.get("cached_tokens"))
                        .and_then(Value::as_u64);
                    let cache_write_tokens = usage
                        .get("input_tokens_details")
                        .and_then(|d| d.get("cache_write_tokens"))
                        .and_then(Value::as_u64)
                        .or_else(|| {
                            usage
                                .get("cache_creation_input_tokens")
                                .and_then(Value::as_u64)
                        });
                    let reasoning_tokens = usage
                        .get("output_tokens_details")
                        .and_then(|d| d.get("reasoning_tokens"))
                        .and_then(Value::as_u64);
                    sink(&ProviderEvent::Usage {
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                        cache_write_tokens,
                        reasoning_tokens,
                    });
                }
            }

            flush_opaque(&mut state.opaque_items, sink);

            let finish_reason = if state.saw_tool_call {
                "tool_calls"
            } else if ev_type == "response.incomplete" {
                "length"
            } else {
                "stop"
            };
            sink(&ProviderEvent::Done {
                finish_reason: finish_reason.into(),
            });
            state.saw_done = true;
            return Some(ExitCode::Success);
        }
        "response.failed" | "error" => {
            let err_obj = payload
                .get("response")
                .and_then(|r| r.get("error"))
                .or_else(|| payload.get("error"))
                .unwrap_or(&payload);
            let code = err_obj
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("provider_error")
                .to_owned();
            let message = err_obj
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("upstream error event")
                .to_owned();
            let retryable = matches!(
                code.as_str(),
                "rate_limit_exceeded" | "rate_limit" | "server_error" | "overloaded"
            );
            sink(&ProviderEvent::Error {
                code,
                message: message.clone(),
                retryable,
            });
            eprintln!("{TAG}: {message}");
            return Some(ExitCode::RuntimeError);
        }
        _ => {}
    }

    None
}

fn emit_function_call_item<F>(item: &Value, state: &mut ResponsesStreamState, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned();
    let name = item
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let args_str = item
        .get("arguments")
        .and_then(Value::as_str)
        .unwrap_or("{}");
    if !call_id.is_empty() && !name.is_empty() && state.emitted_call_ids.insert(call_id.clone()) {
        sink(&ProviderEvent::ToolCall {
            id: call_id,
            name,
            arguments: parse_tool_arguments(args_str),
        });
        state.saw_tool_call = true;
    }
}

fn flush_opaque<F>(opaque_items: &mut Vec<Value>, sink: &mut F)
where
    F: FnMut(&ProviderEvent),
{
    if !opaque_items.is_empty() {
        let items = std::mem::take(opaque_items);
        sink(&ProviderEvent::Opaque {
            data: json!({ "responses_reasoning_items": items }),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Cursor;

    use anix_proto::IR_VERSION;
    use anix_proto::ir::{HeaderTag, IrHeader, IrMeta, StderrMode};

    // RFC §3.3-07 — 有 max_output_tokens 的 IR 经 Responses 转换后写入上游请求。
    // RFC §9.1-06 — Responses 请求把 header 的输出预算映射为对应上游参数。
    // RFC §9.1-07 — 在接收的 Responses HTTP 请求中检查推理等参数映射。
    #[test]
    fn builds_responses_request_with_frozen_instructions_and_reasoning_effort() {
        let stream = IrStream {
            header: IrHeader {
                ir_version: IR_VERSION,
                record_type: HeaderTag::Header,
                tools: vec![IrStream::msh_tool_def()],
                max_output_tokens: Some(4096),
                params: BTreeMap::from([("reasoning_effort".into(), json!("medium"))]),
                meta: IrMeta {
                    output_mode: None,
                    stderr_mode: StderrMode::Auto,
                    cwd: "/tmp".into(),
                    turn_budget: None,
                    extra: BTreeMap::new(),
                },
                extra: BTreeMap::new(),
            },
            messages: vec![
                IrMessage::system(0, "FROZEN_SYS"),
                IrMessage::user(1, "hello"),
                IrMessage::system(2, "<runtime_update>ok</runtime_update>"),
            ],
        };

        let body = build_request(&stream, "gpt-5").expect("build responses request");
        assert_eq!(body["instructions"], "FROZEN_SYS");
        assert_eq!(body["max_output_tokens"], 4096);
        assert_eq!(body["reasoning"], json!({"effort": "medium"}));
        assert_eq!(body["input"].as_array().expect("input").len(), 2);
        assert_eq!(body["input"][1]["role"], "user");
    }

    // RFC §9.2-18 — Responses SSE 分别解码正文、工具调用和 usage 帧。
    #[test]
    fn parses_responses_sse_text_tool_and_usage() {
        let sse = concat!(
            "event: response.reasoning_summary_text.delta\n",
            "data: {\"type\":\"response.reasoning_summary_text.delta\",\"delta\":\"plan\"}\n\n",
            "event: response.output_text.delta\n",
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n",
            "event: response.completed\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"output\":[],\"usage\":{\"input_tokens\":100,\"output_tokens\":20,\"input_tokens_details\":{\"cached_tokens\":60},\"output_tokens_details\":{\"reasoning_tokens\":5}}}}\n\n"
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
                    text: "plan".into()
                },
                ProviderEvent::TextDelta {
                    text: "hello".into()
                },
                ProviderEvent::Usage {
                    input_tokens: 100,
                    output_tokens: 20,
                    cached_tokens: Some(60),
                    cache_write_tokens: None,
                    reasoning_tokens: Some(5),
                },
                ProviderEvent::Done {
                    finish_reason: "stop".into()
                },
            ]
        );
    }
}
