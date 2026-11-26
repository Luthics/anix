use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode as StdExitCode;

use anix_proto::exit_code::ExitCode;
use msh::exec::{ExecOptions, ExecOutcome};
use msh::{ansi, binary, exec, fold};

const BG_PREVIEW_MAX_LINES: usize = 40;
const BG_PREVIEW_MAX_BYTES: usize = 4096;

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("msh {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::Success.into();
    }
    // Internal background reaper callback invoked by JOB_WRAPPER upon completion.
    if args.len() == 8 && args[1] == "--reap-notify" {
        let job_id = &args[2];
        let exit_code = args[3].parse::<i32>().unwrap_or(1);
        let log_path = Path::new(&args[4]);
        let inbox_dir = Path::new(&args[5]);
        let sent_flag = Path::new(&args[6]);
        let cmd = &args[7];
        exec::deliver_reap_notify_once(job_id, exit_code, log_path, inbox_dir, sent_flag, cmd);
        return ExitCode::Success.into();
    }
    if args.len() != 3 || args[1] != "-c" {
        eprintln!("Usage: msh -c \"<command_string>\"");
        return ExitCode::UsageError.into();
    }

    let opts = ExecOptions::from_env();
    let outcome = match exec::run_command_with_options(&args[2], &opts) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("msh: {err}");
            return ExitCode::RuntimeError.into();
        }
    };

    match outcome {
        ExecOutcome::Completed {
            raw_output,
            exit_code,
        } => {
            let Some(output) = format_completed_output(&raw_output, exit_code) else {
                return ExitCode::RuntimeError.into();
            };
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(output.as_bytes());
            let _ = stdout.flush();
            StdExitCode::from((exit_code & 0xFF) as u8)
        }
        ExecOutcome::TimedOutKilled {
            raw_output,
            timeout_sec,
        } => {
            let exit_code = ExitCode::TimeoutOrBudget.as_i32();
            let Some(output) = format_completed_output(&raw_output, exit_code) else {
                return ExitCode::RuntimeError.into();
            };
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(output.as_bytes());
            if !output.is_empty() && !output.ends_with('\n') {
                let _ = writeln!(stdout);
            }
            let _ = writeln!(
                stdout,
                "[msh: command timed out after {timeout_sec}s; process group terminated (exit=124)]"
            );
            let _ = stdout.flush();
            ExitCode::TimeoutOrBudget.into()
        }
        ExecOutcome::Signaled {
            raw_output,
            exit_code,
        } => {
            let Some(output) = format_completed_output(&raw_output, exit_code) else {
                return ExitCode::RuntimeError.into();
            };
            let mut stdout = io::stdout().lock();
            let _ = stdout.write_all(output.as_bytes());
            let _ = stdout.flush();
            StdExitCode::from((exit_code & 0xFF) as u8)
        }
        ExecOutcome::Backgrounded {
            job_id,
            pid,
            pgid,
            log_path,
            exit_file,
            preview_bytes,
            waited_sec,
            immediate,
        } => {
            let mut stdout = io::stdout().lock();
            if !preview_bytes.is_empty() {
                let preview = format_bg_preview(&preview_bytes);
                if !preview.is_empty() {
                    let _ = stdout.write_all(preview.as_bytes());
                    if !preview.ends_with('\n') {
                        let _ = writeln!(stdout);
                    }
                }
            }
            if immediate {
                let _ = writeln!(
                    stdout,
                    "[msh: started in background (job={job_id}, pid={pid}, pgid={pgid})]"
                );
            } else {
                let _ = writeln!(
                    stdout,
                    "[msh: still running after {waited_sec}s — moved to background (job={job_id}, pid={pid}, pgid={pgid})]"
                );
            }
            let _ = writeln!(
                stdout,
                "[msh: log={} | exit_file={} | stop: kill -TERM -{pgid}]",
                log_path.display(),
                exit_file.display()
            );
            let _ = stdout.flush();
            ExitCode::Success.into()
        }
    }
}

fn format_completed_output(raw_output: &[u8], exit_code: i32) -> Option<String> {
    match binary::inspect_or_suppress(raw_output) {
        Ok(binary::OutputKind::BinarySuppressed { bytes, mime, path }) => {
            let mut line = binary::format_suppression_line(bytes, mime, &path);
            line.push('\n');
            Some(line)
        }
        Ok(binary::OutputKind::Text(text)) => {
            let cleaned = ansi::clean_ansi_and_cr(text);
            let limits = fold::FoldLimits::from_env();
            match fold::fold_if_needed(&cleaned, exit_code, limits) {
                Ok(folded) => Some(folded),
                Err(err) => {
                    eprintln!("msh: failed to persist folded output: {err}");
                    None
                }
            }
        }
        Err(err) => {
            eprintln!("msh: failed to persist binary output: {err}");
            None
        }
    }
}

fn format_bg_preview(raw_bytes: &[u8]) -> String {
    let probe = &raw_bytes[..raw_bytes.len().min(8192)];
    if probe.contains(&0) {
        return "[msh: binary output detected so far; inspect log file directly]".to_owned();
    }
    let text = String::from_utf8_lossy(raw_bytes);
    let cleaned = ansi::clean_ansi_and_cr(&text);
    let lines: Vec<&str> = cleaned.lines().collect();
    let tail_slice = if lines.len() > BG_PREVIEW_MAX_LINES {
        &lines[lines.len() - BG_PREVIEW_MAX_LINES..]
    } else {
        &lines[..]
    };
    let joined = tail_slice.join("\n");
    if joined.len() <= BG_PREVIEW_MAX_BYTES {
        return joined;
    }
    let mut start = joined.len() - BG_PREVIEW_MAX_BYTES;
    while start < joined.len() && !joined.is_char_boundary(start) {
        start += 1;
    }
    format!("...[earlier output in log]...\n{}", &joined[start..])
}
