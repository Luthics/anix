use std::process::ExitCode as StdExitCode;

use anix_core::cli;

fn main() -> StdExitCode {
    cli::run().into()
}
