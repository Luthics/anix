//! Shared HTTP/SSE transport, environment loading, lazy media Base64 encoding,
//! and JSONL event emission used across all built-in Provider dialects (Sections 9.1 & 9.2).

use std::fs;
use std::io::{self, BufRead, Write};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{ALLOWED_MEDIA_MIMES, MediaAttachment};
use os_agent_proto::provider::ProviderEvent;
use serde_json::{Value, json};
use thiserror::Error;

use crate::Dialect;

/// Errors raised while translating Canonical IR v1 into an upstream provider payload.
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

/// Provider runtime parameters read from injected environment variables (Section 9.2).
#[derive(Debug, Clone)]
pub struct ProviderEnv {
    pub base_url: String,
    pub api_key: String,
    pub timeout_sec: u64,
    pub model: String,
}

impl ProviderEnv {
    pub fn from_env(dialect: Dialect) -> Result<Self, (ExitCode, &'static str, String)> {
        let base_url = std::env::var("AGENT_PROVIDER_BASE_URL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| dialect.default_base_url().to_owned());
        // Empty API key is permitted for local/intranet unauthenticated endpoints (Section 10.3).
        let api_key = std::env::var("AGENT_PROVIDER_API_KEY").unwrap_or_default();
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

/// Emit a single JSONL [`ProviderEvent`] to `stdout` and flush immediately.
pub fn emit_event(event: &ProviderEvent) {
    if let Ok(line) = serde_json::to_string(event) {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

/// Lazily validate MIME type and read a local media file from disk into standard Base64 (Section 9.1).
pub fn read_media_base64(media: &MediaAttachment) -> Result<String, TranslateError> {
    if !ALLOWED_MEDIA_MIMES.contains(&media.mime.as_str()) {
        return Err(TranslateError::UnsupportedMime(media.mime.clone()));
    }
    let bytes = fs::read(&media.path).map_err(|source| TranslateError::MediaNotFound {
        path: media.path.clone(),
        source,
    })?;
    Ok(BASE64_STD.encode(bytes))
}

/// Parse tool call JSON arguments, tolerating empty strings and invalid JSON safely.
pub fn parse_tool_arguments(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| json!({ "raw": trimmed }))
}

/// Send ONE single-shot HTTP POST request (zero implicit retries; Section 9.2).
///
/// On non-2xx HTTP status or transport/serialization error, emits the corresponding
/// `ProviderEvent::Error`, prints to `stderr`, and returns the mapped `ExitCode`.
pub fn send_single_shot_post<F>(
    tag: &str,
    url: &str,
    timeout_sec: u64,
    extra_headers: &[(&str, String)],
    body: &Value,
    sink: &mut F,
) -> Result<ureq::http::Response<ureq::Body>, ExitCode>
where
    F: FnMut(&ProviderEvent),
{
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout_sec)))
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();

    let json_body = match serde_json::to_string(body) {
        Ok(s) => s,
        Err(err) => {
            let msg = format!("failed to serialize request body: {err}");
            sink(&ProviderEvent::Error {
                code: "serialize_error".into(),
                message: msg.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {msg}");
            return Err(ExitCode::RuntimeError);
        }
    };

    let mut req = agent
        .post(url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");
    for (k, v) in extra_headers {
        req = req.header(*k, v.as_str());
    }

    let response = match req.send(json_body.as_bytes()) {
        Ok(resp) => resp,
        Err(err) => {
            let is_timeout = matches!(err, ureq::Error::Timeout(_));
            let code = if is_timeout {
                "timeout"
            } else {
                "network_error"
            };
            let msg = err.to_string();
            sink(&ProviderEvent::Error {
                code: code.into(),
                message: msg.clone(),
                retryable: true,
            });
            eprintln!("{tag}: {msg}");
            return Err(if is_timeout {
                ExitCode::TimeoutOrBudget
            } else {
                ExitCode::RuntimeError
            });
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
        } else if status == 408 || status == 529 || status >= 500 {
            ("server_error", true)
        } else {
            ("client_error", false)
        };
        let snippet: String = err_body.chars().take(512).collect();
        let msg = format!("HTTP {status}: {snippet}");
        sink(&ProviderEvent::Error {
            code: code.into(),
            message: msg.clone(),
            retryable,
        });
        eprintln!("{tag}: {msg}");
        return Err(ExitCode::RuntimeError);
    }

    Ok(response)
}

/// Drive a standard Server-Sent Events (`text/event-stream`) line reader, invoking `on_frame`
/// with `(event_name, joined_data)` for each complete SSE frame.
///
/// Returns `Ok(Some(code))` if `on_frame` terminated early, `Ok(None)` on clean EOF,
/// or `Err(ExitCode::RuntimeError)` on stream I/O error (after emitting `stream_interrupted`).
pub fn drive_sse_reader<R, F, S>(
    tag: &str,
    mut reader: R,
    sink: &mut S,
    mut on_frame: F,
) -> Result<Option<ExitCode>, ExitCode>
where
    R: BufRead,
    F: FnMut(&str, &str, &mut S) -> Option<ExitCode>,
    S: FnMut(&ProviderEvent),
{
    let mut line = String::new();
    let mut event_name = String::new();
    let mut data_lines: Vec<String> = Vec::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => {
                if !data_lines.is_empty()
                    && let Some(exit) = on_frame(&event_name, &data_lines.join("\n"), sink)
                {
                    return Ok(Some(exit));
                }
                return Ok(None);
            }
            Ok(_) => {
                let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
                if trimmed.is_empty() {
                    if !data_lines.is_empty() {
                        let data = data_lines.join("\n");
                        let ev = std::mem::take(&mut event_name);
                        data_lines.clear();
                        if let Some(exit) = on_frame(&ev, &data, sink) {
                            return Ok(Some(exit));
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
                sink(&ProviderEvent::Error {
                    code: "stream_interrupted".into(),
                    message: msg.clone(),
                    retryable: true,
                });
                eprintln!("{tag}: {msg}");
                return Err(ExitCode::RuntimeError);
            }
        }
    }
}
