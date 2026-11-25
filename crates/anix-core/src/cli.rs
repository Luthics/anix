//! CLI argument parsing & dispatch (Section 3.1).

use std::path::PathBuf;

use anix_proto::{IR_VERSION, exit_code::ExitCode};
use clap::{Parser, Subcommand, ValueEnum};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputArg {
    Reply,
    Explicit,
    None,
}

#[derive(Debug, Parser)]
#[command(
    name = "anix",
    about = "Anix — POSIX-native Agent runtime",
    disable_version_flag = true,
    args_conflicts_with_subcommands = true
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

    /// Static `fd 1` (`stdout`) delivery contract: `reply` (default), `explicit` (`$ANIX_OUT`), or `none` (v1.2.0).
    #[arg(short = 'o', long = "output", value_enum)]
    pub output: Option<OutputArg>,

    /// Explicitly declare that the active model supports native vision (`vision=true`).
    #[arg(long = "vision", overrides_with = "no_vision")]
    pub vision: bool,

    /// Explicitly declare that the active model does NOT support native vision (`vision=false`).
    #[arg(long = "no-vision", overrides_with = "vision")]
    pub no_vision: bool,

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

    /// Exit with code 75 (`EX_TEMPFAIL`) immediately when `approval=deny` blocks a command.
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

    /// Explicit writable file descriptor number (`>= 3`) for structured JSONL Trace output (v1.1 §4.1).
    #[arg(long = "trace-fd", value_name = "N")]
    pub trace_fd: Option<i32>,

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

    /// Positional script arguments (passed directly or after `--`).
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
            "anix {} (ir_version={})",
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
        eprintln!("anix: missing <SCRIPT.agent> (try `anix --help`)");
        return ExitCode::UsageError;
    };

    let script = match frontmatter::parse_script_file(script_path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("anix: {err}");
            return match err {
                frontmatter::FrontmatterError::Io { .. } => ExitCode::RuntimeError,
                _ => ExitCode::UsageError,
            };
        }
    };

    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("anix: cannot determine cwd: {err}");
            return ExitCode::RuntimeError;
        }
    };

    let paths = match AgentPaths::discover(&cwd) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("anix: {err}");
            return ExitCode::UsageError;
        }
    };

    let resolved = match ResolvedConfig::resolve(&cli, &script, paths) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("anix: {err}");
            return ExitCode::UsageError;
        }
    };

    crate::runtime::execute(&resolved, &script, &cli.script_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_script_and_args_with_double_dash() {
        let cli = Cli::try_parse_from(["anix", "-y", "-q", "audit.agent", "--", "HEAD~1"])
            .expect("parse cli");
        assert!(cli.yes);
        assert!(cli.quiet);
        assert_eq!(cli.script, Some(PathBuf::from("audit.agent")));
        assert_eq!(cli.script_args, vec!["HEAD~1"]);

        let cli = Cli::try_parse_from(["anix", "bash.agent", "-y", "--", "-q", "-n", "5"])
            .expect("parse hyphenated args after --");
        assert!(cli.yes);
        assert!(!cli.quiet);
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec!["-q", "-n", "5"]);
    }

    #[test]
    fn parses_direct_and_quoted_script_args_without_double_dash() {
        let cli = Cli::try_parse_from(["anix", "bash.agent", "git log -n 5"])
            .expect("parse quoted string with embedded flags");
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec!["git log -n 5"]);

        let cli =
            Cli::try_parse_from(["anix", "bash.agent", "-y", "-c", "", "check", "disk", "-q"])
                .expect("parse mixed flags and direct positional args");
        assert!(cli.yes);
        assert!(cli.quiet);
        assert_eq!(cli.context.as_deref(), Some(""));
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec!["check", "disk"]);

        let cli =
            Cli::try_parse_from(["anix", "bash.agent", ""]).expect("parse empty quoted script arg");
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec![""]);
    }

    #[test]
    fn trust_subcommand_does_not_conflict_with_script_args() {
        let cli = Cli::try_parse_from(["anix", "trust", "./proj"]).expect("parse trust");
        assert!(
            matches!(cli.command, Some(SubCommand::Trust { ref path }) if path == &PathBuf::from("./proj"))
        );
        assert_eq!(cli.script, None);
        assert!(cli.script_args.is_empty());

        let cli = Cli::try_parse_from(["anix", "./bash.agent", "trust", "me"])
            .expect("parse script arg literally named trust");
        assert!(cli.command.is_none());
        assert_eq!(cli.script, Some(PathBuf::from("./bash.agent")));
        assert_eq!(cli.script_args, vec!["trust", "me"]);
    }
}
