//! CLI argument parsing & dispatch (RFC §3.1).

use std::path::PathBuf;

use anix_proto::{IR_VERSION, exit_code::ExitCode};
use clap::{Parser, Subcommand, ValueEnum};

use crate::config::{AgentPaths, ResolvedConfig};
use crate::frontmatter;

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
    about = "Anix — Pure 3-primitive POSIX-native Agent kernel",
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

    /// Explicit path to a single `config.toml` file (overrides `$ANIX_CONFIG` and default).
    #[arg(long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Persistent Tree WAL JSONL context file path (empty string forces stateless mode).
    #[arg(short = 'c', long = "context")]
    pub context: Option<String>,

    /// Target historical WAL node ID (`>= 1`) to branch from (requires `-c <FILE>`).
    #[arg(long = "leaf", value_name = "NODE_ID")]
    pub leaf: Option<u64>,

    /// Wait for WAL `flock` instead of failing immediately (`--lock-wait` or `--lock-wait=<SEC>`).
    #[arg(
        long = "lock-wait",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "infinite",
        value_name = "SEC"
    )]
    pub lock_wait: Option<String>,

    #[arg(long = "provider")]
    pub provider: Option<String>,

    #[arg(long = "model")]
    pub model: Option<String>,

    /// Shell executable or wrapper chain (e.g. `msh`, `anix-approve -- msh`, or `none`).
    #[arg(long = "shell")]
    pub shell: Option<String>,

    /// Static `fd 1` (`stdout`) delivery contract: `reply` (default), `explicit` (`$ANIX_OUT`), or `none`.
    #[arg(short = 'o', long = "output", value_enum)]
    pub output: Option<OutputArg>,

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

    /// Pass-through provider parameter (`KEY=VALUE`, repeatable; value parsed as JSON or string).
    #[arg(long = "param", value_name = "KEY=VALUE")]
    pub params: Vec<String>,

    #[arg(long = "stderr", value_enum)]
    pub stderr: Option<StderrArg>,

    /// Equivalent to `--stderr explicit`.
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Explicit writable file descriptor number (`>= 3`) for structured JSONL Trace output.
    #[arg(long = "trace-fd", value_name = "N")]
    pub trace_fd: Option<i32>,

    /// Maildir-style Inbox queue directory (`--inbox <DIR>`). Never read from `ANIX_INBOX`.
    #[arg(long = "inbox")]
    pub inbox: Option<PathBuf>,

    /// Idle auto-exit timeout in seconds for `--inbox` mode (default: 300; 0 = never exit).
    #[arg(long = "idle-timeout")]
    pub idle_timeout: Option<u64>,

    /// Debounce window in milliseconds to coalesce concurrent Maildir arrivals (default: 100).
    #[arg(long = "inbox-debounce-ms")]
    pub inbox_debounce_ms: Option<u64>,

    /// Deliver an atomic exit notification message to `<MAILDIR>` before process termination.
    #[arg(long = "notify", value_name = "MAILDIR")]
    pub notify: Option<PathBuf>,

    /// Optional correlation ID written to `Anix-In-Reply-To` in `--notify` messages.
    #[arg(long = "notify-id", value_name = "ID")]
    pub notify_id: Option<String>,

    /// Enable Linux Landlock sandbox (`--sandbox` or `--sandbox=strict`).
    #[arg(
        long = "sandbox",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "best-effort"
    )]
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
    /// Atomically deliver a message to a Maildir inbox (optionally waiting for a synchronous reply).
    Send {
        /// Target Maildir directory.
        maildir: PathBuf,
        /// Create a temporary reply Maildir, wait for the agent's reply, and print it to stdout.
        #[arg(long = "wait")]
        wait: bool,
        /// Maximum seconds to wait when `--wait` is enabled (exit 124 on timeout).
        #[arg(long = "timeout", value_name = "SEC")]
        timeout: Option<u64>,
        /// Correlated lease or message ID (`Anix-In-Reply-To`).
        #[arg(long = "in-reply-to", value_name = "ID")]
        in_reply_to: Option<String>,
        /// Kernel control operation such as `compact` (`Anix-Control`).
        #[arg(long = "control", value_name = "OP")]
        control: Option<String>,
        /// Optional sender label (`Anix-From`).
        #[arg(long = "from", value_name = "LABEL")]
        from: Option<String>,
        /// Message body text (reads from stdin if omitted and stdin is not a TTY).
        message: Vec<String>,
    },
    /// Fork a Tree WAL file into a new independent WAL file referencing `<SRC>#<LEAF>` read-only.
    Fork {
        /// Source Tree WAL file path.
        src: PathBuf,
        /// Source node ID (`>= 1`) to branch from (defaults to active EOF leaf).
        #[arg(long = "leaf", value_name = "NODE_ID")]
        leaf: Option<u64>,
        /// Destination path for the newly created forked WAL file (must not already exist).
        #[arg(short = 'o', long = "output", value_name = "DST_WAL")]
        out: PathBuf,
    },
    /// Verify integrity of a WAL file (UTF-8, JSON, ir_version, monotonic IDs, topology, tool closure, branch_base).
    Fsck {
        /// WAL file path to inspect.
        wal: PathBuf,
    },
    /// Print Tree WAL topology or active branch lineage summary.
    Log {
        /// WAL file path to inspect.
        wal: PathBuf,
        /// Optional node ID (`>= 1`) to display only its chronological lineage (including across forks).
        #[arg(long = "leaf", value_name = "NODE_ID")]
        leaf: Option<u64>,
    },
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
        return crate::doctor::run(cli.config.as_deref());
    }

    if let Some(cmd) = cli.command {
        return match cmd {
            SubCommand::Send {
                maildir,
                wait,
                timeout,
                in_reply_to,
                control,
                from,
                message,
            } => crate::maildir::run_send(
                &maildir,
                wait,
                timeout,
                in_reply_to.as_deref(),
                control.as_deref(),
                from.as_deref(),
                &message,
            ),
            SubCommand::Fork { src, leaf, out } => crate::wal::fork_wal(&src, leaf, &out),
            SubCommand::Fsck { wal } => crate::wal::fsck_wal(&wal),
            SubCommand::Log { wal, leaf } => crate::wal::log_wal(&wal, leaf),
        };
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
        let cli =
            Cli::try_parse_from(["anix", "-q", "audit.agent", "--", "HEAD~1"]).expect("parse cli");
        assert!(cli.quiet);
        assert_eq!(cli.script, Some(PathBuf::from("audit.agent")));
        assert_eq!(cli.script_args, vec!["HEAD~1"]);

        let cli = Cli::try_parse_from(["anix", "bash.agent", "--", "-q", "-n", "5"])
            .expect("parse hyphenated args after --");
        assert!(!cli.quiet);
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec!["-q", "-n", "5"]);
    }

    #[test]
    fn parses_lock_wait_notify_and_subcommands() {
        let cli = Cli::try_parse_from([
            "anix",
            "--lock-wait",
            "--notify",
            "/tmp/cb",
            "--notify-id",
            "job-1",
            "-c",
            "sess.jsonl",
            "--leaf",
            "12",
            "bash.agent",
            "hi",
        ])
        .expect("parse lock-wait without value before script");
        assert_eq!(cli.lock_wait.as_deref(), Some("infinite"));
        assert_eq!(cli.notify, Some(PathBuf::from("/tmp/cb")));
        assert_eq!(cli.notify_id.as_deref(), Some("job-1"));
        assert_eq!(cli.leaf, Some(12));
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));
        assert_eq!(cli.script_args, vec!["hi"]);

        let cli = Cli::try_parse_from(["anix", "--lock-wait=15", "bash.agent"])
            .expect("parse --lock-wait=15");
        assert_eq!(cli.lock_wait.as_deref(), Some("15"));
        assert_eq!(cli.script, Some(PathBuf::from("bash.agent")));

        let cli = Cli::try_parse_from([
            "anix",
            "send",
            "/tmp/inbox",
            "--wait",
            "--timeout",
            "30",
            "--from",
            "orchestrator",
            "hello",
            "world",
        ])
        .expect("parse send");
        assert!(matches!(
            cli.command,
            Some(SubCommand::Send {
                wait: true,
                timeout: Some(30),
                ..
            })
        ));

        let cli = Cli::try_parse_from([
            "anix",
            "fork",
            "src.jsonl",
            "--leaf",
            "4",
            "-o",
            "dst.jsonl",
        ])
        .expect("parse fork");
        assert!(matches!(
            cli.command,
            Some(SubCommand::Fork { leaf: Some(4), .. })
        ));

        let cli = Cli::try_parse_from(["anix", "fsck", "sess.jsonl"]).expect("parse fsck");
        assert!(matches!(cli.command, Some(SubCommand::Fsck { .. })));

        let cli =
            Cli::try_parse_from(["anix", "log", "sess.jsonl", "--leaf", "5"]).expect("parse log");
        assert!(matches!(
            cli.command,
            Some(SubCommand::Log { leaf: Some(5), .. })
        ));

        // Script positional arg literally named "send" should still work.
        let cli = Cli::try_parse_from(["anix", "./bash.agent", "send", "report"])
            .expect("parse script arg literally named send");
        assert!(cli.command.is_none());
        assert_eq!(cli.script, Some(PathBuf::from("./bash.agent")));
        assert_eq!(cli.script_args, vec!["send", "report"]);
    }
}
