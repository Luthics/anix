//! `aplan`: live task checklist updates over `$ANIX_CTL` (ephemeral UI progress, zero WAL bloat).
//!
//! Accepts either Markdown checkboxes on `stdin` (most natural for LLMs):
//! ```sh
//! aplan <<'EOF'
//! - [x] Inspect auth middleware
//! - [~] Fix token expiry check
//! - [ ] Run cargo test
//! EOF
//! ```
//! or concise CLI flags (`aplan -d "Step 1" -i "Step 2" -p "Step 3"` / `aplan clear`).

use std::io::{self, Read};
use std::process::ExitCode as StdExitCode;

use anix_proto::ctl::{CtlFrame, PlanStep, PlanStepStatus};
use anix_proto::exit_code::ExitCode;
use anix_tools::send_ctl_frame;

const MAX_PLAN_STEPS: usize = 24;
const MAX_TITLE_CHARS: usize = 120;

const USAGE: &str = "\
Usage:
  aplan <<'EOF'               Read Markdown checklist from stdin (`[x]` done, `[~]`/`[>]` in_progress, `[ ]` pending, `[!]` failed, `[-]` skipped)
  aplan -d <title> -i <title> -p <title> [-s <title>] [-f <title>]
                              Set checklist steps inline via flags
  aplan clear                 Clear the active checklist";

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.len() == 1 && (args[0] == "--version" || args[0] == "-V") {
        println!("aplan {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::Success.into();
    }
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{USAGE}");
        return ExitCode::Success.into();
    }

    let steps = if args.len() == 1 && matches!(args[0].as_str(), "clear" | "--clear" | "reset") {
        Vec::new()
    } else if args.is_empty() || (args.len() == 1 && args[0] == "-") {
        let mut raw = String::new();
        if let Err(err) = io::stdin().read_to_string(&mut raw) {
            eprintln!("aplan: failed reading stdin: {err}");
            return ExitCode::RuntimeError.into();
        }
        let parsed = parse_markdown_checklist(&raw);
        if parsed.is_empty() && !raw.trim().is_empty() {
            eprintln!("aplan: no valid checklist items found on stdin\n{USAGE}");
            return ExitCode::UsageError.into();
        }
        parsed
    } else {
        match parse_flag_args(&args) {
            Ok(list) => list,
            Err(msg) => {
                eprintln!("aplan: {msg}\n{USAGE}");
                return ExitCode::UsageError.into();
            }
        }
    };

    let done_count = steps
        .iter()
        .filter(|s| s.status == PlanStepStatus::Done)
        .count();
    let total = steps.len();

    if let Err(err) = send_ctl_frame(&CtlFrame::Plan { steps }) {
        eprintln!("aplan: {err}");
        return ExitCode::RuntimeError.into();
    }

    if total == 0 {
        println!("[aplan] cleared plan");
    } else {
        println!("[aplan] updated plan ({done_count}/{total} done)");
    }
    ExitCode::Success.into()
}

fn clean_title(raw: &str) -> String {
    let single = raw.replace(['\r', '\n'], " ");
    let trimmed = single.trim();
    let mut out = String::new();
    for (idx, ch) in trimmed.chars().enumerate() {
        if idx >= MAX_TITLE_CHARS {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn parse_markdown_checklist(raw: &str) -> Vec<PlanStep> {
    let mut steps = Vec::new();
    for line in raw.lines() {
        if steps.len() >= MAX_PLAN_STEPS {
            break;
        }
        let mut s = line.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        // Strip optional list marker (`- `, `* `, `+ `, or `1. ` / `1) `).
        if let Some(rest) = s
            .strip_prefix("- ")
            .or_else(|| s.strip_prefix("* "))
            .or_else(|| s.strip_prefix("+ "))
        {
            s = rest.trim_start();
        } else {
            let digits = s.bytes().take_while(|b| b.is_ascii_digit()).count();
            if digits > 0
                && let Some(after) = s[digits..]
                    .strip_prefix(". ")
                    .or_else(|| s[digits..].strip_prefix(") "))
            {
                s = after.trim_start();
            }
        }

        // Expect `[<marker>] <title>` (also accept plain text lines as `pending`).
        let (status, rest) = if let Some(after_open) = s.strip_prefix('[')
            && let Some(close_idx) = after_open.find(']')
        {
            let marker = after_open[..close_idx].trim();
            let title_part = after_open[close_idx + 1..].trim();
            let status = match marker {
                "x" | "X" | "✓" | "v" | "done" => PlanStepStatus::Done,
                "~" | ">" | "*" | "▸" | "in_progress" | "wip" => PlanStepStatus::InProgress,
                "!" | "err" | "fail" | "failed" => PlanStepStatus::Failed,
                "-" | "skip" | "skipped" => PlanStepStatus::Skipped,
                "" | " " | "o" | "○" | "pending" => PlanStepStatus::Pending,
                _ => PlanStepStatus::Pending,
            };
            (status, title_part)
        } else {
            (PlanStepStatus::Pending, s)
        };

        let title = clean_title(rest);
        if !title.is_empty() {
            steps.push(PlanStep { title, status });
        }
    }
    steps
}

fn parse_flag_args(args: &[String]) -> Result<Vec<PlanStep>, String> {
    let mut steps = Vec::new();
    let mut i = 0usize;
    while i < args.len() {
        let status = match args[i].as_str() {
            "-d" | "--done" => PlanStepStatus::Done,
            "-i" | "--in-progress" | "--wip" => PlanStepStatus::InProgress,
            "-p" | "--pending" => PlanStepStatus::Pending,
            "-s" | "--skipped" => PlanStepStatus::Skipped,
            "-f" | "--failed" => PlanStepStatus::Failed,
            other => {
                // Allow bare shorthand `"title:status"` or plain pending title
                if other.starts_with('-') {
                    return Err(format!("unknown flag `{other}`"));
                }
                let (t_raw, st) = if let Some((lhs, rhs)) = other.rsplit_once(':') {
                    match rhs.trim().to_ascii_lowercase().as_str() {
                        "done" | "x" => (lhs, PlanStepStatus::Done),
                        "in_progress" | "wip" | "active" => (lhs, PlanStepStatus::InProgress),
                        "pending" | "todo" => (lhs, PlanStepStatus::Pending),
                        "skipped" | "skip" => (lhs, PlanStepStatus::Skipped),
                        "failed" | "fail" => (lhs, PlanStepStatus::Failed),
                        _ => (other, PlanStepStatus::Pending),
                    }
                } else {
                    (other, PlanStepStatus::Pending)
                };
                let title = clean_title(t_raw);
                if !title.is_empty() && steps.len() < MAX_PLAN_STEPS {
                    steps.push(PlanStep { title, status: st });
                }
                i += 1;
                continue;
            }
        };
        let Some(raw_title) = args.get(i + 1) else {
            return Err(format!("flag `{}` requires a step title", args[i]));
        };
        let title = clean_title(raw_title);
        if !title.is_empty() && steps.len() < MAX_PLAN_STEPS {
            steps.push(PlanStep { title, status });
        }
        i += 2;
    }
    if steps.is_empty() {
        return Err("no steps specified".into());
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_markdown_checkboxes_and_flags() {
        let md = "\
# Plan
- [x] Inspect auth middleware
* [~] Fix token expiry check
3. [ ] Add regression test
- [-] Skip legacy v1 path
- [!] Check flaky integration test
";
        let steps = parse_markdown_checklist(md);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].status, PlanStepStatus::Done);
        assert_eq!(steps[0].title, "Inspect auth middleware");
        assert_eq!(steps[1].status, PlanStepStatus::InProgress);
        assert_eq!(steps[2].status, PlanStepStatus::Pending);
        assert_eq!(steps[3].status, PlanStepStatus::Skipped);
        assert_eq!(steps[4].status, PlanStepStatus::Failed);

        let flagged = parse_flag_args(&[
            "-d".into(),
            "Read code".into(),
            "-i".into(),
            "Edit code".into(),
            "-p".into(),
            "Run tests".into(),
        ])
        .unwrap();
        assert_eq!(flagged.len(), 3);
        assert_eq!(flagged[0].status, PlanStepStatus::Done);
        assert_eq!(flagged[1].status, PlanStepStatus::InProgress);
        assert_eq!(flagged[2].status, PlanStepStatus::Pending);
    }
}
