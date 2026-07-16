use std::{io, path::Path, process::ExitCode};

use clap::Parser;
use dexdeck::{Cli, DexdeckExitCode, TerminalCapabilities, execute};
use dexdeck_config::write_text_atomic;
use dexdeck_core::{Clock, DebugDiagnostics, DebugLevel, SecretRedactor, SystemClock};

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
    let debug_log = cli.debug_log_path().map(Path::to_path_buf);
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let code = execute(cli, terminal, &mut stdout, &mut stderr);
    drop(stdout);
    drop(stderr);

    let (code, detail) = if code != DexdeckExitCode::Success {
        (code, None)
    } else if let Some(options) = shell_options {
        match dexdeck_tui::run(options) {
            Ok(()) => (DexdeckExitCode::Success, None),
            Err(error) => {
                let message = error.to_string();
                eprintln!("dexdeck: {message}");
                (DexdeckExitCode::Internal, Some(message))
            }
        }
    } else {
        (DexdeckExitCode::Success, None)
    };

    if let Some(path) = debug_log
        && let Err(error) = persist_debug_log(&path, code, detail.as_deref())
    {
        eprintln!("dexdeck: failed to write debug log: {error}");
        return ExitCode::from(DexdeckExitCode::Internal as u8);
    }
    ExitCode::from(code as u8)
}

fn persist_debug_log(
    path: &Path,
    code: DexdeckExitCode,
    detail: Option<&str>,
) -> Result<(), dexdeck_config::StorageError> {
    let mut diagnostics = DebugDiagnostics::default();
    let redactor = SecretRedactor::new();
    let now = SystemClock.now_ms();
    diagnostics.push(now, DebugLevel::Info, "cli", "DexDeck started", &redactor);
    if let Some(detail) = detail {
        diagnostics.push(now, DebugLevel::Error, "tui", detail, &redactor);
    }
    diagnostics.push(
        now,
        DebugLevel::Info,
        "cli",
        &format!("DexDeck exited with code {}", code as u8),
        &redactor,
    );
    write_text_atomic(path, &diagnostics.render_text())
}
