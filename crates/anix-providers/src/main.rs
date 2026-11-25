use std::io::{self, BufRead, BufReader, Read};
use std::process::ExitCode as StdExitCode;

use anix_proto::IR_VERSION;
use anix_proto::exit_code::ExitCode;
use anix_proto::ir::{IrError, IrStream};
use anix_proto::provider::{ProviderEvent, ProviderMode, ProviderRequestEnvelope};
use anix_providers::common::{ProviderEnv, TranslateError, build_http_agent, emit_event};
use anix_providers::{Dialect, Invocation, parse_invocation};

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().collect();
    let env_dialect = std::env::var("ANIX_PROVIDER_DIALECT").ok();

    let dialect = match parse_invocation(&args, env_dialect.as_deref()) {
        Ok(Invocation::Version(Some(d))) => {
            println!(
                "{} {} (ir_version={})",
                d.bin_name(),
                env!("CARGO_PKG_VERSION"),
                IR_VERSION
            );
            return ExitCode::Success.into();
        }
        Ok(Invocation::Version(None)) => {
            let names: Vec<&str> = Dialect::ALL.iter().map(|d| d.name()).collect();
            println!(
                "anix-provider {} (ir_version={}; dialects={})",
                env!("CARGO_PKG_VERSION"),
                IR_VERSION,
                names.join(",")
            );
            return ExitCode::Success.into();
        }
        Ok(Invocation::Help) => {
            println!("Usage: anix-provider [<dialect> | --dialect <dialect>] [--version]");
            println!("       anix-provider-<dialect> [--version]");
            println!();
            println!(
                "Modes (via ANIX_PROVIDER_MODE=coprocess|oneshot, or auto-detected from stdin):"
            );
            println!(
                "  coprocess          Persistent JSONL request loop reusing HTTP/TLS connections (v1.1 default)"
            );
            println!(
                "  oneshot            Single Canonical IR v1 JSONL stream terminated by EOF (v1.0 compat)"
            );
            println!();
            println!("Built-in dialects:");
            for d in Dialect::ALL {
                println!("  {:18} (symlink: {})", d.name(), d.bin_name());
            }
            return ExitCode::Success.into();
        }
        Ok(Invocation::Run(d)) => d,
        Err(message) => {
            emit_event(&ProviderEvent::Error {
                code: "dialect_error".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("anix-provider: {message}");
            return ExitCode::UsageError.into();
        }
    };

    let tag = dialect.bin_name();

    let explicit_mode = match std::env::var("ANIX_PROVIDER_MODE") {
        Ok(raw) if !raw.trim().is_empty() => match ProviderMode::parse_str(&raw) {
            Ok(m) => Some(m),
            Err(err) => {
                let message = format!("invalid ANIX_PROVIDER_MODE: {err}");
                emit_event(&ProviderEvent::Error {
                    code: "invalid_mode".into(),
                    message: message.clone(),
                    retryable: false,
                });
                eprintln!("{tag}: {message}");
                return ExitCode::UsageError.into();
            }
        },
        _ => None,
    };

    match explicit_mode {
        Some(ProviderMode::Coprocess) => run_coprocess(dialect, tag, None),
        Some(ProviderMode::Oneshot) => run_oneshot(dialect, tag, None),
        None => run_auto_detect(dialect, tag),
    }
    .into()
}

fn run_auto_detect(dialect: Dialect, tag: &str) -> ExitCode {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut first_line = String::new();
    loop {
        first_line.clear();
        match reader.read_line(&mut first_line) {
            Ok(0) => {
                // Empty EOF on stdin: match v1.0 oneshot error behavior.
                drop(reader);
                return run_oneshot(dialect, tag, Some(String::new()));
            }
            Ok(_) => {
                if first_line.trim().is_empty() {
                    continue;
                }
                break;
            }
            Err(err) => {
                let message = format!("failed to read stdin: {err}");
                emit_event(&ProviderEvent::Error {
                    code: "stdin_read_error".into(),
                    message: message.clone(),
                    retryable: false,
                });
                eprintln!("{tag}: {message}");
                return ExitCode::RuntimeError;
            }
        }
    }

    let is_coprocess_envelope = serde_json::from_str::<serde_json::Value>(first_line.trim())
        .ok()
        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
        .as_deref()
        == Some("request");

    if is_coprocess_envelope {
        run_coprocess_reader(dialect, tag, Some(first_line), &mut reader)
    } else {
        let mut raw_ir = first_line;
        if let Err(err) = reader.read_to_string(&mut raw_ir) {
            let message = format!("failed to read Canonical IR from stdin: {err}");
            emit_event(&ProviderEvent::Error {
                code: "stdin_read_error".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return ExitCode::RuntimeError;
        }
        run_oneshot(dialect, tag, Some(raw_ir))
    }
}

fn run_oneshot(dialect: Dialect, tag: &str, pre_read: Option<String>) -> ExitCode {
    let env = match ProviderEnv::from_env_for_mode(dialect, ProviderMode::Oneshot) {
        Ok(env) => env,
        Err((exit, code, message)) => {
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return exit;
        }
    };

    let raw_ir = match pre_read {
        Some(s) => s,
        None => {
            let mut s = String::new();
            if let Err(err) = io::stdin().read_to_string(&mut s) {
                let message = format!("failed to read Canonical IR from stdin: {err}");
                emit_event(&ProviderEvent::Error {
                    code: "stdin_read_error".into(),
                    message: message.clone(),
                    retryable: false,
                });
                eprintln!("{tag}: {message}");
                return ExitCode::RuntimeError;
            }
            s
        }
    };

    let stream = match IrStream::from_jsonl(&raw_ir) {
        Ok(s) => s,
        Err(IrError::UnsupportedVersion(v)) => {
            let message = format!("unsupported ir_version {v} (expected {IR_VERSION})");
            emit_event(&ProviderEvent::Error {
                code: "unsupported_ir_version".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return ExitCode::UsageError;
        }
        Err(err) => {
            let message = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: "invalid_ir".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return ExitCode::UsageError;
        }
    };

    if let Err(err) = stream.validate_invariants() {
        let message = err.to_string();
        emit_event(&ProviderEvent::Error {
            code: "invalid_ir".into(),
            message: message.clone(),
            retryable: false,
        });
        eprintln!("{tag}: {message}");
        return ExitCode::UsageError;
    }

    let body = match dialect.build_request(&stream, &env.model) {
        Ok(body) => body,
        Err(err) => {
            let code = match &err {
                TranslateError::MediaNotFound { .. } => "media_not_found",
                TranslateError::UnsupportedMime(_) => "unsupported_media_mime",
            };
            let message = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return ExitCode::RuntimeError;
        }
    };

    dialect.execute_streaming(&env, &stream, &body)
}

fn run_coprocess(dialect: Dialect, tag: &str, first_line: Option<String>) -> ExitCode {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    run_coprocess_reader(dialect, tag, first_line, &mut reader)
}

fn run_coprocess_reader<R: BufRead>(
    dialect: Dialect,
    tag: &str,
    mut pending_first: Option<String>,
    reader: &mut R,
) -> ExitCode {
    let base_env = match ProviderEnv::from_env_for_mode(dialect, ProviderMode::Coprocess) {
        Ok(env) => env,
        Err((exit, code, message)) => {
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return exit;
        }
    };

    // Single reusable `ureq::Agent` shared across all requests in this coprocess lifetime
    // to pool TCP/TLS connections (v1.1 §9.2).
    let http_agent = build_http_agent(base_env.timeout_sec);
    let mut line_buf = String::new();

    loop {
        let raw_line = if let Some(first) = pending_first.take() {
            first
        } else {
            line_buf.clear();
            match reader.read_line(&mut line_buf) {
                Ok(0) => {
                    // Clean EOF on stdin: graceful shutdown with exit 0 (v1.1 §9.2).
                    return ExitCode::Success;
                }
                Ok(_) => line_buf.clone(),
                Err(err) => {
                    let message = format!("failed to read coprocess request from stdin: {err}");
                    emit_event(&ProviderEvent::Error {
                        code: "stdin_read_error".into(),
                        message: message.clone(),
                        retryable: false,
                    });
                    eprintln!("{tag}: {message}");
                    return ExitCode::RuntimeError;
                }
            }
        };

        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let envelope: ProviderRequestEnvelope = match serde_json::from_str(trimmed) {
            Ok(env) => env,
            Err(err) => {
                let message = format!("invalid coprocess request envelope: {err}");
                emit_event(&ProviderEvent::Error {
                    code: "invalid_request".into(),
                    message: message.clone(),
                    retryable: false,
                });
                eprintln!("{tag}: {message}");
                continue;
            }
        };

        if envelope.ir_version != IR_VERSION {
            let message = format!(
                "unsupported ir_version {} (expected {IR_VERSION})",
                envelope.ir_version
            );
            emit_event(&ProviderEvent::Error {
                code: "unsupported_ir_version".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            continue;
        }

        let req_env = base_env.with_overrides(envelope.model.as_deref(), envelope.timeout_sec);
        if req_env.model.trim().is_empty() {
            let message = "model is missing in both coprocess request and ANIX_MODEL".to_owned();
            emit_event(&ProviderEvent::Error {
                code: "model_missing".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            continue;
        }

        let stream = envelope.into_ir_stream();
        if let Err(err) = stream.validate_invariants() {
            let message = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: "invalid_ir".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            continue;
        }

        let body = match dialect.build_request(&stream, &req_env.model) {
            Ok(body) => body,
            Err(err) => {
                let code = match &err {
                    TranslateError::MediaNotFound { .. } => "media_not_found",
                    TranslateError::UnsupportedMime(_) => "unsupported_media_mime",
                };
                let message = err.to_string();
                emit_event(&ProviderEvent::Error {
                    code: code.into(),
                    message: message.clone(),
                    retryable: false,
                });
                eprintln!("{tag}: {message}");
                continue;
            }
        };

        // Always keep coprocess alive after each request (whether `done` or `error`);
        // `execute_streaming_with_agent` guarantees a terminal `done` or `error` frame.
        let _ = dialect.execute_streaming_with_agent(&http_agent, &req_env, &stream, &body);
    }
}
