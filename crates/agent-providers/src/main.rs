use std::io::{self, Read};
use std::process::ExitCode as StdExitCode;

use agent_providers::common::{ProviderEnv, TranslateError, emit_event};
use agent_providers::{Dialect, Invocation, parse_invocation};
use os_agent_proto::IR_VERSION;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{IrError, IrStream};
use os_agent_proto::provider::ProviderEvent;

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().collect();
    let env_dialect = std::env::var("AGENT_PROVIDER_DIALECT").ok();

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
                "agent-provider {} (ir_version={}; dialects={})",
                env!("CARGO_PKG_VERSION"),
                IR_VERSION,
                names.join(",")
            );
            return ExitCode::Success.into();
        }
        Ok(Invocation::Help) => {
            println!("Usage: agent-provider [<dialect> | --dialect <dialect>] [--version]");
            println!("       agent-provider-<dialect> [--version]");
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
            eprintln!("agent-provider: {message}");
            return ExitCode::UsageError.into();
        }
    };

    let tag = dialect.bin_name();

    let env = match ProviderEnv::from_env(dialect) {
        Ok(env) => env,
        Err((exit, code, message)) => {
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return exit.into();
        }
    };

    let mut raw_ir = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut raw_ir) {
        let message = format!("failed to read Canonical IR from stdin: {err}");
        emit_event(&ProviderEvent::Error {
            code: "stdin_read_error".into(),
            message: message.clone(),
            retryable: false,
        });
        eprintln!("{tag}: {message}");
        return ExitCode::RuntimeError.into();
    }

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
            return ExitCode::UsageError.into();
        }
        Err(err) => {
            let message = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: "invalid_ir".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("{tag}: {message}");
            return ExitCode::UsageError.into();
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
        return ExitCode::UsageError.into();
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
            return ExitCode::RuntimeError.into();
        }
    };

    dialect.execute_streaming(&env, &stream, &body).into()
}
