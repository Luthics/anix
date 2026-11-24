use std::io::{self, Write};
use std::process::ExitCode as StdExitCode;

use msh::{ansi, binary, exec, fold};
use os_agent_proto::exit_code::ExitCode;

fn main() -> StdExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && (args[1] == "--version" || args[1] == "-V") {
        println!("msh {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::Success.into();
    }
    if args.len() != 3 || args[1] != "-c" {
        eprintln!("Usage: msh -c \"<command_string>\"");
        return ExitCode::UsageError.into();
    }

    let timeout_sec = exec::timeout_from_env();
    let res = match exec::run_command(&args[2], timeout_sec) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("msh: {err}");
            return ExitCode::RuntimeError.into();
        }
    };

    let output = match binary::inspect_or_suppress(&res.raw_output) {
        Ok(binary::OutputKind::BinarySuppressed { bytes, mime, path }) => {
            let mut line = binary::format_suppression_line(bytes, mime, &path);
            line.push('\n');
            line
        }
        Ok(binary::OutputKind::Text(text)) => {
            let cleaned = ansi::clean_ansi_and_cr(text);
            let limits = fold::FoldLimits::from_env();
            match fold::fold_if_needed(&cleaned, res.exit_code, limits) {
                Ok(folded) => folded,
                Err(err) => {
                    eprintln!("msh: failed to persist folded output: {err}");
                    return ExitCode::RuntimeError.into();
                }
            }
        }
        Err(err) => {
            eprintln!("msh: failed to persist binary output: {err}");
            return ExitCode::RuntimeError.into();
        }
    };

    let mut stdout = io::stdout().lock();
    let _ = stdout.write_all(output.as_bytes());
    if res.timed_out {
        let _ = writeln!(
            stdout,
            "[msh: command timed out after {}s; process group terminated (exit=124)]",
            res.timeout_sec
        );
    }
    let _ = stdout.flush();

    StdExitCode::from((res.exit_code & 0xFF) as u8)
}
