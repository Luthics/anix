//! `aclr`: explicit context compaction / tail folding over `$ANIX_CTL` (Sections 6.1 & 7.2).

use std::process::ExitCode as StdExitCode;

use anix_proto::{ctl::CtlFrame, exit_code::ExitCode};
use anix_tools::send_ctl_frame;

const USAGE: &str = "\
Usage:
  aclr -s                         Trigger full compaction and write a new WAL checkpoint
  aclr -t <turn_id> [<summary>]   Fold tail turns [turn_id..=now] (auto-summarizes via [compact] when summary is omitted)";

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::UsageError.into();
    }

    if args.len() == 1 && (args[0] == "--version" || args[0] == "-V") {
        println!("aclr {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::Success.into();
    }

    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::Success.into();
    }

    let frame = match args[0].as_str() {
        "-s" if args.len() == 1 => CtlFrame::Compact,
        "-t" => {
            if args.len() < 2 {
                eprintln!("aclr: `-t` requires a `<turn_id>`\n{USAGE}");
                return ExitCode::UsageError.into();
            }
            let Ok(from_turn) = args[1].parse::<u64>() else {
                eprintln!("aclr: invalid turn_id `{}`\n{USAGE}", args[1]);
                return ExitCode::UsageError.into();
            };
            if from_turn == 0 {
                eprintln!(
                    "aclr: turn_id must be >= 1 (turn 0 is the frozen system prefix)\n{USAGE}"
                );
                return ExitCode::UsageError.into();
            }
            let summary = if args.len() >= 3 {
                let joined = args[2..].join(" ");
                if joined.trim().is_empty() {
                    eprintln!("aclr: summary must not be empty when provided\n{USAGE}");
                    return ExitCode::UsageError.into();
                }
                Some(joined)
            } else {
                None
            };
            CtlFrame::Fold { from_turn, summary }
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::UsageError.into();
        }
    };

    if let Err(err) = send_ctl_frame(&frame) {
        eprintln!("aclr: {err}");
        return ExitCode::RuntimeError.into();
    }

    match &frame {
        CtlFrame::Compact => println!("[aclr] scheduled full context compaction"),
        CtlFrame::Fold {
            from_turn,
            summary: Some(_),
        } => {
            println!("[aclr] scheduled tail fold from turn {from_turn}")
        }
        CtlFrame::Fold {
            from_turn,
            summary: None,
        } => {
            println!("[aclr] scheduled auto-summary tail fold from turn {from_turn}")
        }
        CtlFrame::Media { .. } | CtlFrame::Hold { .. } | CtlFrame::Release { .. } => {}
    }

    ExitCode::Success.into()
}
