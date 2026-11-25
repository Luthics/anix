//! Bidirectional translation between Canonical IR v1 JSONL and OpenAI Responses API JSON (Sections 9.1 & 9.2).

use std::fs;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use os_agent_proto::ir::{ALLOWED_MEDIA_MIMES, IrMessage, IrStream, MediaAttachment};
use serde_json::{Map, Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranslateError {
    #[error("media file not found or unreadable at `{path}`: {source}")]
    MediaNotFound {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("unsupported media MIME type `{0}`")]
    UnsupportedMime(String),
}

/// Build an OpenAI `/v1/responses` JSON request payload from a Canonical IR v1 stream.
///
/// Preserves the first `role: "system"` message byte-for-byte in `instructions`
/// to maximize upstream KV-cache prefix hits, and lazily reads local media
/// attachments from disk into Base64 data URIs.
pub fn build_responses_request(stream: &IrStream, model: &str) -> Result<Value, TranslateError> {
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
                // Replay any opaque reasoning items persisted from this Provider.
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
                // Always emit plain string `output` in `function_call_output` and flush any
                // attached visual media in a follow-up `role: "user"` item right after the
                // contiguous tool-output batch. vLLM / OpenAI-compatible Responses endpoints
                // silently drop `input_image` parts inside `function_call_output`, whereas
                // `role: "user"` `input_image` works uniformly across OpenAI, DeepSeek, and vLLM.
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

    // Omit `tools` completely when empty (e.g. during full compaction, Section 9.1).
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

    // Section 9.1: Map `max_output_tokens` directly when present and >= 1.
    if let Some(max_out) = stream.header.max_output_tokens
        && max_out >= 1
    {
        body.insert("max_output_tokens".into(), Value::from(max_out));
    }

    // Section 9.1: Map pass-through `params`; warn & ignore unsupported keys (never fail).
    for (key, val) in &stream.header.params {
        match key.as_str() {
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
            | "user"
            | "prompt_cache_key" => {
                body.insert(key.clone(), val.clone());
            }
            other => {
                eprintln!(
                    "agent-provider-openai-responses: warning: ignoring unsupported param `{other}`"
                );
            }
        }
    }

    Ok(Value::Object(body))
}

fn encode_media_part(media: &MediaAttachment) -> Result<Value, TranslateError> {
    if !ALLOWED_MEDIA_MIMES.contains(&media.mime.as_str()) {
        return Err(TranslateError::UnsupportedMime(media.mime.clone()));
    }
    let bytes = fs::read(&media.path).map_err(|source| TranslateError::MediaNotFound {
        path: media.path.clone(),
        source,
    })?;
    let b64 = BASE64_STD.encode(bytes);
    let data_uri = format!("data:{};base64,{b64}", media.mime);
    Ok(json!({
        "type": "input_image",
        "image_url": data_uri
    }))
}
