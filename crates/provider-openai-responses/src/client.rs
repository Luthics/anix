//! Stateless single-shot HTTP/SSE client for OpenAI Responses API (Section 9.2).
//!
//! Strictly performs ONE request with ZERO implicit retries; retryable HTTP
//! 429/5xx or transport errors are emitted as `{"type":"error","retryable":true}`
//! and exited non-zero so `agent` owns exponential backoff and trace logging.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::provider::ProviderEvent;
use serde_json::{Value, json};

/// Provider runtime parameters read from injected environment variables.
#[derive(Debug, Clone)]
pub struct ProviderEnv {
    pub base_url: String,
    pub api_key: String,
    pub timeout_sec: u64,
    pub model: String,
}

impl ProviderEnv {
    pub fn from_env() -> Result<Self, (ExitCode, &'static str, String)> {
        let base_url = std::env::var("AGENT_PROVIDER_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.openai.com/v1".into());
        let api_key = std::env::var("AGENT_PROVIDER_API_KEY").unwrap_or_default();
        if api_key.trim().is_empty() {
            return Err((
                ExitCode::RuntimeError,
                "auth_missing",
                "AGENT_PROVIDER_API_KEY is empty or unset".into(),
            ));
        }
        let model = std::env::var("AGENT_MODEL").unwrap_or_default();
        if model.trim().is_empty() {
            return Err((
                ExitCode::UsageError,
                "model_missing",
                "AGENT_MODEL is empty or unset".into(),
            ));
        }
        let timeout_sec = std::env::var("AGENT_PROVIDER_TIMEOUT_SEC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(120);

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            api_key,
            timeout_sec,
            model,
        })
    }
}

/// Emit a single JSONL ProviderEvent to `stdout` and flush immediately.
pub fn emit_event(event: &ProviderEvent) {
    if let Ok(line) = serde_json::to_string(event) {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

#[derive(Debug, Default)]
struct PendingFunctionCall {
    call_id: String,
    name: String,
    arguments: String,
}

/// Execute a single-shot streaming POST to `{base_url}/responses` and stream
/// canonical JSONL events to `stdout`.
pub fn execute_streaming_request(env: &ProviderEnv, body: &Value) -> ExitCode {
    let url = format!("{}/responses", env.base_url);
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(env.timeout_sec)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let json_body = match serde_json::to_string(body) {
        Ok(s) => s,
        Err(err) => {
            let msg = format!("failed to serialize request body: {err}");
            emit_event(&ProviderEvent::Error {
                code: "serialize_error".into(),
                message: msg.clone(),
                retryable: false,
            });
            eprintln!("agent-provider-openai-responses: {msg}");
            return ExitCode::RuntimeError;
        }
    };

    let response = match agent
        .post(&url)
        .header("Authorization", &format!("Bearer {}", env.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .send(json_body.as_bytes())
    {
        Ok(resp) => resp,
        Err(err) => {
            let is_timeout = matches!(err, ureq::Error::Timeout(_));
            let code = if is_timeout {
                "timeout"
            } else {
                "network_error"
            };
            let msg = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: msg.clone(),
                retryable: true,
            });
            eprintln!("agent-provider-openai-responses: {msg}");
            return if is_timeout {
                ExitCode::TimeoutOrBudget
            } else {
                ExitCode::RuntimeError
            };
        }
    };

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let err_body = response
            .into_body()
            .read_to_string()
            .unwrap_or_else(|_| "<unreadable error body>".into());
        let (code, retryable) = if status == 429 {
            ("rate_limit", true)
        } else if status == 408 || status >= 500 {
            ("server_error", true)
        } else {
            ("client_error", false)
        };
        let snippet: String = err_body.chars().take(512).collect();
        let msg = format!("HTTP {status}: {snippet}");
        emit_event(&ProviderEvent::Error {
            code: code.into(),
            message: msg.clone(),
            retryable,
        });
        eprintln!("agent-provider-openai-responses: {msg}");
        return ExitCode::RuntimeError;
    }

    let reader = BufReader::new(response.into_body().into_reader());
    consume_sse_stream(reader)
}

fn consume_sse_stream<R: BufRead>(mut reader: R) -> ExitCode {
    let mut line = String::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    let mut pending_calls: HashMap<String, PendingFunctionCall> = HashMap::new();
    let mut emitted_call_ids: HashSet<String> = HashSet::new();
    let mut opaque_items: Vec<Value> = Vec::new();
    let mut saw_tool_call = false;
    let mut saw_done = false;

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // Flush trailing frame if stream ended without trailing blank line.
                if !data_lines.is_empty()
                    && let Some(exit) = dispatch_sse_frame(
                        &event_name,
                        &data_lines.join("\n"),
                        &mut pending_calls,
                        &mut emitted_call_ids,
                        &mut opaque_items,
                        &mut saw_tool_call,
                        &mut saw_done,
                    )
                {
                    return exit;
                }
                break;
            }
            Ok(_) => {
                let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
                if trimmed.is_empty() {
                    if !data_lines.is_empty() {
                        let data = data_lines.join("\n");
                        let ev = std::mem::take(&mut event_name);
                        data_lines.clear();
                        if let Some(exit) = dispatch_sse_frame(
                            &ev,
                            &data,
                            &mut pending_calls,
                            &mut emitted_call_ids,
                            &mut opaque_items,
                            &mut saw_tool_call,
                            &mut saw_done,
                        ) {
                            return exit;
                        }
                    } else {
                        event_name.clear();
                    }
                    continue;
                }

                if trimmed.starts_with(':') {
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("event:") {
                    event_name = rest.trim().to_owned();
                } else if let Some(rest) = trimmed.strip_prefix("data:") {
                    data_lines.push(rest.strip_prefix(' ').unwrap_or(rest).to_owned());
                }
            }
            Err(err) => {
                let msg = format!("SSE stream read error: {err}");
                emit_event(&ProviderEvent::Error {
                    code: "stream_interrupted".into(),
                    message: msg.clone(),
                    retryable: true,
                });
                eprintln!("agent-provider-openai-responses: {msg}");
                return ExitCode::RuntimeError;
            }
        }
    }

    if saw_done {
        ExitCode::Success
    } else {
        let msg = "upstream SSE stream closed without terminal completion event";
        emit_event(&ProviderEvent::Error {
            code: "stream_truncated".into(),
            message: msg.into(),
            retryable: true,
        });
        eprintln!("agent-provider-openai-responses: {msg}");
        ExitCode::RuntimeError
    }
}

fn dispatch_sse_frame(
    sse_event: &str,
    data: &str,
    pending_calls: &mut HashMap<String, PendingFunctionCall>,
    emitted_call_ids: &mut HashSet<String>,
    opaque_items: &mut Vec<Value>,
    saw_tool_call: &mut bool,
    saw_done: &mut bool,
) -> Option<ExitCode> {
    if data.trim() == "[DONE]" {
        if !*saw_done {
            flush_opaque(opaque_items);
            let finish_reason = if *saw_tool_call { "tool_calls" } else { "stop" };
            emit_event(&ProviderEvent::Done {
                finish_reason: finish_reason.into(),
            });
            *saw_done = true;
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
                emit_event(&ProviderEvent::TextDelta {
                    text: delta.to_owned(),
                });
            }
        }
        "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
            if let Some(delta) = payload.get("delta").and_then(Value::as_str)
                && !delta.is_empty()
            {
                emit_event(&ProviderEvent::ReasoningDelta {
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
                    pending_calls.insert(
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
                && let Some(entry) = pending_calls.get_mut(item_id)
            {
                entry.arguments.push_str(delta);
            }
        }
        "response.function_call_arguments.done" => {
            if let Some(item_id) = payload.get("item_id").and_then(Value::as_str)
                && let Some(entry) = pending_calls.get_mut(item_id)
            {
                if let Some(args) = payload.get("arguments").and_then(Value::as_str) {
                    entry.arguments = args.to_owned();
                }
                if !entry.call_id.is_empty()
                    && !entry.name.is_empty()
                    && emitted_call_ids.insert(entry.call_id.clone())
                {
                    let parsed_args = parse_tool_arguments(&entry.arguments);
                    emit_event(&ProviderEvent::ToolCall {
                        id: entry.call_id.clone(),
                        name: entry.name.clone(),
                        arguments: parsed_args,
                    });
                    *saw_tool_call = true;
                }
            }
        }
        "response.output_item.done" => {
            if let Some(item) = payload.get("item") {
                match item.get("type").and_then(Value::as_str) {
                    Some("function_call") => {
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
                        if !call_id.is_empty()
                            && !name.is_empty()
                            && emitted_call_ids.insert(call_id.clone())
                        {
                            emit_event(&ProviderEvent::ToolCall {
                                id: call_id,
                                name,
                                arguments: parse_tool_arguments(args_str),
                            });
                            *saw_tool_call = true;
                        }
                    }
                    Some("reasoning") if item.get("encrypted_content").is_some() => {
                        opaque_items.push(item.clone());
                    }
                    _ => {}
                }
            }
        }
        "response.completed" | "response.incomplete" => {
            if let Some(resp) = payload.get("response") {
                // Fallback scan for any function_call or reasoning items not caught earlier.
                if let Some(outputs) = resp.get("output").and_then(Value::as_array) {
                    for item in outputs {
                        if item.get("type").and_then(Value::as_str) == Some("function_call") {
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
                            if !call_id.is_empty()
                                && !name.is_empty()
                                && emitted_call_ids.insert(call_id.clone())
                            {
                                emit_event(&ProviderEvent::ToolCall {
                                    id: call_id,
                                    name,
                                    arguments: parse_tool_arguments(args_str),
                                });
                                *saw_tool_call = true;
                            }
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
                    emit_event(&ProviderEvent::Usage {
                        input_tokens,
                        output_tokens,
                        cached_tokens,
                    });
                }
            }

            flush_opaque(opaque_items);

            let finish_reason = if *saw_tool_call {
                "tool_calls"
            } else if ev_type == "response.incomplete" {
                "length"
            } else {
                "stop"
            };
            emit_event(&ProviderEvent::Done {
                finish_reason: finish_reason.into(),
            });
            *saw_done = true;
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
            emit_event(&ProviderEvent::Error {
                code,
                message: message.clone(),
                retryable,
            });
            eprintln!("agent-provider-openai-responses: {message}");
            return Some(ExitCode::RuntimeError);
        }
        _ => {}
    }

    None
}

fn flush_opaque(opaque_items: &mut Vec<Value>) {
    if !opaque_items.is_empty() {
        let items = std::mem::take(opaque_items);
        emit_event(&ProviderEvent::Opaque {
            data: json!({ "responses_reasoning_items": items }),
        });
    }
}

fn parse_tool_arguments(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| json!({ "raw": trimmed }))
}
