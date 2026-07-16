use std::{io, process::ExitCode};

use clap::Parser;
use dexdeck::{Cli, TerminalCapabilities, execute};

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return ExitCode::from(code);
        }
    };

    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    ExitCode::from(execute(
        cli,
        TerminalCapabilities::detect(),
        &mut stdout,
        &mut stderr,
    ) as u8)
}
