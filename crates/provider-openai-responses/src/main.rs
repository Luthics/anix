use std::io::{self, Read};
use std::process::ExitCode as StdExitCode;

use os_agent_proto::IR_VERSION;
use os_agent_proto::exit_code::ExitCode;
use os_agent_proto::ir::{IrError, IrStream};
use os_agent_proto::provider::ProviderEvent;
use provider_openai_responses::client::{ProviderEnv, emit_event, execute_streaming_request};
use provider_openai_responses::translate::{TranslateError, build_responses_request};

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!(
            "agent-provider-openai-responses {} (ir_version={})",
            env!("CARGO_PKG_VERSION"),
            IR_VERSION
        );
        return ExitCode::Success.into();
    }

    let env = match ProviderEnv::from_env() {
        Ok(env) => env,
        Err((exit, code, message)) => {
            emit_event(&ProviderEvent::Error {
                code: code.into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("agent-provider-openai-responses: {message}");
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
        eprintln!("agent-provider-openai-responses: {message}");
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
            eprintln!("agent-provider-openai-responses: {message}");
            return ExitCode::UsageError.into();
        }
        Err(err) => {
            let message = err.to_string();
            emit_event(&ProviderEvent::Error {
                code: "invalid_ir".into(),
                message: message.clone(),
                retryable: false,
            });
            eprintln!("agent-provider-openai-responses: {message}");
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
        eprintln!("agent-provider-openai-responses: {message}");
        return ExitCode::UsageError.into();
    }

    let body = match build_responses_request(&stream, &env.model) {
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
            eprintln!("agent-provider-openai-responses: {message}");
            return ExitCode::RuntimeError.into();
        }
    };

    execute_streaming_request(&env, &body).into()
}
