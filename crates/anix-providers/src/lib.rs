//! Unified stateless Canonical IR v1 Provider adapter (`anix-provider`, Section 9.2).
//!
//! Dispatches to built-in HTTP/SSE protocol dialects using POSIX BusyBox `argv[0]`
//! symlink names (with optional CLI subcommand / `--dialect` / `ANIX_PROVIDER_DIALECT`
//! fallback):
//! - `anix-provider-openai-responses` → OpenAI Responses API (`/v1/responses`)
//! - `anix-provider-openai-compat` → OpenAI-compatible Chat Completions API (`/v1/chat/completions`)
//! - `anix-provider-anthropic` → Anthropic Messages API (`/v1/messages`)

pub mod anthropic;
pub mod common;
pub mod openai_compat;
pub mod openai_responses;

use std::path::Path;

use anix_proto::exit_code::ExitCode;
use anix_proto::ir::IrStream;

use crate::common::{ProviderEnv, TranslateError};

/// Built-in HTTP/SSE wire protocol dialects supported by `anix-provider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    OpenAiResponses,
    OpenAiCompat,
    Anthropic,
}

impl Dialect {
    /// All built-in dialects in canonical order.
    pub const ALL: &'static [Self] = &[Self::OpenAiResponses, Self::OpenAiCompat, Self::Anthropic];

    /// Short canonical dialect identifier (matches `[providers.<name>]`).
    pub const fn name(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai-responses",
            Self::OpenAiCompat => "openai-compat",
            Self::Anthropic => "anthropic",
        }
    }

    /// BusyBox symlink binary name (`anix-provider-<dialect>`).
    pub const fn bin_name(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "anix-provider-openai-responses",
            Self::OpenAiCompat => "anix-provider-openai-compat",
            Self::Anthropic => "anix-provider-anthropic",
        }
    }

    /// Default upstream API base URL when `ANIX_PROVIDER_BASE_URL` is unset/empty.
    pub const fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAiResponses | Self::OpenAiCompat => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com",
        }
    }

    /// Parse a dialect name or BusyBox symlink basename (`anix-provider-<dialect>`).
    pub fn from_name(raw: &str) -> Option<Self> {
        let base = Path::new(raw)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(raw);
        let stripped = base
            .strip_suffix(".exe")
            .unwrap_or(base)
            .strip_prefix("anix-provider-")
            .unwrap_or(base);
        match stripped {
            "openai-responses" => Some(Self::OpenAiResponses),
            "openai-compat" => Some(Self::OpenAiCompat),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    /// Build the upstream HTTP request body from a validated Canonical IR v1 stream.
    pub fn build_request(
        self,
        stream: &IrStream,
        model: &str,
    ) -> Result<serde_json::Value, TranslateError> {
        self.build_request_with_session(stream, model, None)
    }

    /// Build the upstream HTTP request body with optional session routing affinity ID (v1.1.1 §9.2).
    pub fn build_request_with_session(
        self,
        stream: &IrStream,
        model: &str,
        session_id: Option<&str>,
    ) -> Result<serde_json::Value, TranslateError> {
        match self {
            Self::OpenAiResponses => {
                openai_responses::build_request_with_session(stream, model, session_id)
            }
            Self::OpenAiCompat => {
                openai_compat::build_request_with_session(stream, model, session_id)
            }
            Self::Anthropic => anthropic::build_request(stream, model),
        }
    }

    /// Execute one stateless streaming HTTP request and emit `ProviderEvent` JSONL to `stdout`.
    pub fn execute_streaming(
        self,
        env: &ProviderEnv,
        stream: &IrStream,
        body: &serde_json::Value,
    ) -> ExitCode {
        let agent = common::build_http_agent(env.timeout_sec);
        self.execute_streaming_with_agent(&agent, env, stream, body)
    }

    /// Execute one streaming HTTP request reusing a pooled `ureq::Agent` (coprocess mode).
    pub fn execute_streaming_with_agent(
        self,
        agent: &ureq::Agent,
        env: &ProviderEnv,
        stream: &IrStream,
        body: &serde_json::Value,
    ) -> ExitCode {
        match self {
            Self::OpenAiResponses => openai_responses::execute_streaming_with_agent(
                agent,
                env,
                stream,
                body,
                common::emit_event,
            ),
            Self::OpenAiCompat => openai_compat::execute_streaming_with_agent(
                agent,
                env,
                stream,
                body,
                common::emit_event,
            ),
            Self::Anthropic => anthropic::execute_streaming_with_agent(
                agent,
                env,
                stream,
                body,
                common::emit_event,
            ),
        }
    }
}

/// Parsed CLI invocation outcome for `anix-provider`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Version(Option<Dialect>),
    Help,
    Run(Dialect),
}

/// Resolve dialect and CLI control flags from `argv` and optional `ANIX_PROVIDER_DIALECT`.
pub fn parse_invocation(args: &[String], env_dialect: Option<&str>) -> Result<Invocation, String> {
    let argv0 = args.first().map(String::as_str).unwrap_or("anix-provider");
    let mut dialect = Dialect::from_name(argv0);

    let mut idx = 1;
    while idx < args.len() {
        let arg = args[idx].as_str();
        match arg {
            "--version" | "-V" => return Ok(Invocation::Version(dialect)),
            "--help" | "-h" => return Ok(Invocation::Help),
            "--dialect" => {
                idx += 1;
                let Some(val) = args.get(idx) else {
                    return Err("--dialect requires a value".into());
                };
                dialect = Some(Dialect::from_name(val).ok_or_else(|| {
                    format!(
                        "unknown dialect `{val}` (expected: openai-responses, openai-compat, anthropic)"
                    )
                })?);
            }
            _ if arg.starts_with("--dialect=") => {
                let val = &arg["--dialect=".len()..];
                dialect = Some(Dialect::from_name(val).ok_or_else(|| {
                    format!(
                        "unknown dialect `{val}` (expected: openai-responses, openai-compat, anthropic)"
                    )
                })?);
            }
            other if dialect.is_none() => {
                dialect = Some(Dialect::from_name(other).ok_or_else(|| {
                    format!(
                        "unknown dialect or argument `{other}` (expected: openai-responses, openai-compat, anthropic)"
                    )
                })?);
            }
            other => {
                return Err(format!("unexpected argument `{other}`"));
            }
        }
        idx += 1;
    }

    if dialect.is_none()
        && let Some(raw) = env_dialect.filter(|s| !s.trim().is_empty())
    {
        dialect = Some(Dialect::from_name(raw.trim()).ok_or_else(|| {
            format!(
                "unknown ANIX_PROVIDER_DIALECT `{raw}` (expected: openai-responses, openai-compat, anthropic)"
            )
        })?);
    }

    match dialect {
        Some(d) => Ok(Invocation::Run(d)),
        None => Err(
            "no provider dialect selected: invoke via symlink `anix-provider-<dialect>`, pass `<dialect>` / `--dialect <dialect>`, or set ANIX_PROVIDER_DIALECT"
                .into(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|x| (*x).to_owned()).collect()
    }

    // RFC v1.1 §2, §9.2
    #[test]
    fn resolves_dialect_from_argv0_symlink_and_version() {
        assert_eq!(
            parse_invocation(
                &s(&["/home/u/.local/libexec/anix/anix-provider-openai-responses"]),
                None
            ),
            Ok(Invocation::Run(Dialect::OpenAiResponses))
        );
        assert_eq!(
            parse_invocation(&s(&["anix-provider-openai-compat"]), None),
            Ok(Invocation::Run(Dialect::OpenAiCompat))
        );
        assert_eq!(
            parse_invocation(&s(&["anix-provider-anthropic", "--version"]), None),
            Ok(Invocation::Version(Some(Dialect::Anthropic)))
        );
    }

    // RFC v1.1 §2, §9.2
    #[test]
    fn resolves_dialect_from_subcommand_flag_and_env() {
        assert_eq!(
            parse_invocation(&s(&["anix-provider", "--version"]), None),
            Ok(Invocation::Version(None))
        );
        assert_eq!(
            parse_invocation(&s(&["anix-provider", "openai-compat"]), None),
            Ok(Invocation::Run(Dialect::OpenAiCompat))
        );
        assert_eq!(
            parse_invocation(&s(&["anix-provider", "--dialect=anthropic"]), None),
            Ok(Invocation::Run(Dialect::Anthropic))
        );
        assert_eq!(
            parse_invocation(&s(&["anix-provider"]), Some("openai-responses")),
            Ok(Invocation::Run(Dialect::OpenAiResponses))
        );
        assert!(parse_invocation(&s(&["anix-provider"]), None).is_err());
    }
}
