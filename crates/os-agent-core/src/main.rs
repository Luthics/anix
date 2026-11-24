use std::process::ExitCode as StdExitCode;

use os_agent_core::cli;

fn main() -> StdExitCode {
    cli::run().into()
}
