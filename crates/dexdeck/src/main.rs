use std::{
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

use clap::Parser;
use dexdeck::{Cli, DexdeckExitCode, TerminalCapabilities, execute};
use dexdeck_config::write_text_atomic;
use dexdeck_core::{
    Clock, DebugLevel, SecretRedactor, SystemClock, record_process_diagnostic,
    render_process_diagnostics,
};

fn main() -> ExitCode {
    #[cfg(windows)]
    if std::env::args_os().nth(1).as_deref()
        == Some(std::ffi::OsStr::new("--dexdeck-internal-process-helper"))
    {
        std::process::exit(dexdeck_core::run_windows_process_helper());
    }
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return ExitCode::from(code);
        }
    };

    let terminal = TerminalCapabilities::detect();
    let redactor = SecretRedactor::new();
    record_process_diagnostic(
        SystemClock.now_ms(),
        DebugLevel::Info,
        "cli",
        "DexDeck started",
        &redactor,
    );
    let shell_options = cli.shell_options();
    let mut shell_logcat = cli.shell_logcat_backend();
    let debug_log = cli.debug_log_path().map(Path::to_path_buf);
    let mut stdout = io::stdout().lock();
    let stderr = io::stderr().lock();
    let mut stderr = DiagnosticWriter::new(stderr, &redactor);
    let code = execute(cli, terminal, &mut stdout, &mut stderr);
    drop(stdout);
    drop(stderr);

    let (code, detail) = if code != DexdeckExitCode::Success {
        (code, None)
    } else if let Some(options) = shell_options {
        match dexdeck_tui::run_with_logcat(options, shell_logcat.take()) {
            Ok(()) => (DexdeckExitCode::Success, None),
            Err(error) => {
                let message = error.to_string();
                record_process_diagnostic(
                    SystemClock.now_ms(),
                    DebugLevel::Error,
                    "tui",
                    &message,
                    &redactor,
                );
                eprintln!("dexdeck: {message}");
                (DexdeckExitCode::Internal, Some(message))
            }
        }
    } else {
        (DexdeckExitCode::Success, None)
    };

    if let Some(path) = debug_log
        && let Err(error) = persist_debug_log(&path, code, detail.as_deref(), &redactor)
    {
        eprintln!("dexdeck: failed to write debug log: {error}");
        return ExitCode::from(DexdeckExitCode::Internal as u8);
    }
    ExitCode::from(code as u8)
}

struct DiagnosticWriter<'a, W> {
    inner: W,
    redactor: &'a SecretRedactor,
}

impl<'a, W> DiagnosticWriter<'a, W> {
    const fn new(inner: W, redactor: &'a SecretRedactor) -> Self {
        Self { inner, redactor }
    }
}

impl<W: Write> Write for DiagnosticWriter<'_, W> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buffer)?;
        if written > 0 {
            record_process_diagnostic(
                SystemClock.now_ms(),
                DebugLevel::Error,
                "operation",
                &String::from_utf8_lossy(&buffer[..written]),
                self.redactor,
            );
        }
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn persist_debug_log(
    path: &Path,
    code: DexdeckExitCode,
    detail: Option<&str>,
    redactor: &SecretRedactor,
) -> Result<(), dexdeck_config::StorageError> {
    let now = SystemClock.now_ms();
    if let Some(detail) = detail {
        record_process_diagnostic(now, DebugLevel::Error, "tui", detail, redactor);
    }
    record_process_diagnostic(
        now,
        DebugLevel::Info,
        "cli",
        &format!("DexDeck exited with code {}", code as u8),
        redactor,
    );
    write_text_atomic(path, &render_process_diagnostics())
}
