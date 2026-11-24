//! CLI argument parsing & dispatch (Section 3.1).

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use os_agent_proto::{IR_VERSION, exit_code::ExitCode};

use crate::config::{AgentPaths, ResolvedConfig};
use crate::frontmatter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ApprovalArg {
    Ask,
    Auto,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum StderrArg {
    Auto,
    Explicit,
}

#[derive(Debug, Parser)]
#[command(
    name = "agent",
    about = "OS Agent — POSIX-native Agent runtime",
    disable_version_flag = true
)]
pub struct Cli {
    /// Print version and supported ir_version.
    #[arg(long = "version", short = 'V')]
    pub version: bool,

    /// Verify environment, permissions, and default Provider connectivity.
    #[arg(long = "doctor")]
    pub doctor: bool,

    /// Persistent WAL JSONL context file path (empty string forces stateless mode).
    #[arg(short = 'c', long = "context")]
    pub context: Option<String>,

    #[arg(long = "provider")]
    pub provider: Option<String>,

    #[arg(long = "model")]
    pub model: Option<String>,

    #[arg(long = "shell")]
    pub shell: Option<String>,

    #[arg(long = "max-turns")]
    pub max_turns: Option<u32>,

    #[arg(long = "max-tokens")]
    pub max_tokens: Option<u32>,

    #[arg(long = "max-output-tokens")]
    pub max_output_tokens: Option<u32>,

    #[arg(long = "compact-threshold")]
    pub compact_threshold: Option<String>,

    #[arg(long = "no-compact")]
    pub no_compact: bool,

    #[arg(long = "approval", value_enum)]
    pub approval: Option<ApprovalArg>,

    /// Equivalent to `--approval auto`.
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Exit with code 42 immediately when `approval=deny` blocks a command.
    #[arg(long = "fail-on-deny")]
    pub fail_on_deny: bool,

    /// Pass-through provider parameter (`KEY=VALUE`, repeatable; value parsed as JSON or string).
    #[arg(long = "param", value_name = "KEY=VALUE")]
    pub params: Vec<String>,

    #[arg(long = "stderr", value_enum)]
    pub stderr: Option<StderrArg>,

    /// Equivalent to `--stderr explicit`.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Maildir-style Inbox queue directory (`--inbox <DIR>`).
    #[arg(long = "inbox")]
    pub inbox: Option<PathBuf>,

    /// Idle auto-exit timeout in seconds for `--inbox` mode (default: 300; 0 = never exit).
    #[arg(long = "idle-timeout")]
    pub idle_timeout: Option<u64>,

    /// Enable Linux Landlock sandbox (`--sandbox` or `--sandbox=strict`).
    #[arg(long = "sandbox", num_args = 0..=1, default_missing_value = "best-effort")]
    pub sandbox: Option<String>,

    #[command(subcommand)]
    pub command: Option<SubCommand>,

    /// Path to the `*.agent` script to execute.
    pub script: Option<PathBuf>,

    /// Positional script arguments passed after `--`.
    #[arg(last = true)]
    pub script_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// Trust project-level `hooks/` and custom executables by recording SHA-256 digests.
    Trust { path: PathBuf },
}

pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return if err.exit_code() == 0 {
                ExitCode::Success
            } else {
                ExitCode::UsageError
            };
        }
    };

    if cli.version {
        println!(
            "agent {} (ir_version={})",
            env!("CARGO_PKG_VERSION"),
            IR_VERSION
        );
        return ExitCode::Success;
    }

    if cli.doctor {
        return crate::doctor::run();
    }

    if let Some(SubCommand::Trust { path }) = cli.command {
        return crate::trust::trust_path(&path);
    }

    let Some(ref script_path) = cli.script else {
        eprintln!("agent: missing <SCRIPT.agent> (try `agent --help`)");
        return ExitCode::UsageError;
    };

    let script = match frontmatter::parse_script_file(script_path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("agent: {err}");
            return match err {
                frontmatter::FrontmatterError::Io { .. } => ExitCode::RuntimeError,
                _ => ExitCode::UsageError,
            };
        }
    };

    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("agent: cannot determine cwd: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let paths = match AgentPaths::discover(&cwd) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("agent: {err}");
            return ExitCode::UsageError;
        }
    };

    let resolved = match ResolvedConfig::resolve(&cli, &script, paths) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("agent: {err}");
            return ExitCode::UsageError;
        }
    };

    crate::runtime::execute(&resolved, &script, &cli.script_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_script_and_args() {
        let cli = Cli::try_parse_from(["agent", "-y", "-q", "audit.agent", "--", "HEAD~1"])
            .expect("parse cli");
        assert!(cli.yes);
        assert!(cli.quiet);
        assert_eq!(cli.script, Some(PathBuf::from("audit.agent")));
        assert_eq!(cli.script_args, vec!["HEAD~1"]);
    }
}
