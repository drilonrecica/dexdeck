use std::{io, process::ExitCode};

use clap::Parser;
use dexdeck::{Cli, DexdeckExitCode, TerminalCapabilities, execute};

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return ExitCode::from(code);
        }
    };

    let terminal = TerminalCapabilities::detect();
    let shell_options = cli.shell_options();
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let code = execute(cli, terminal, &mut stdout, &mut stderr);
    drop(stdout);
    drop(stderr);

    if code != DexdeckExitCode::Success {
        return ExitCode::from(code as u8);
    }
    let Some(options) = shell_options else {
        return ExitCode::SUCCESS;
    };
    match dexdeck_tui::run(options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dexdeck: {error}");
            ExitCode::from(DexdeckExitCode::Internal as u8)
        }
    }
}
