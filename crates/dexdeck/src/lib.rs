use std::{
    fmt,
    io::{self, IsTerminal, Write},
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use dexdeck_config::{ProjectIdentity, StoragePaths, load_model};
use dexdeck_gradle::{AdapterKind, discover_project, select_adapter};
use dexdeck_protocol::{CLI_SCHEMA_VERSION, ProjectModel};
use dexdeck_tui::ShellOptions;
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "dexdeck",
    bin_name = "dexdeck",
    version,
    about = "A fast, private terminal control plane for Android development",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[arg(long, global = true, value_name = "PATH")]
    project: Option<std::path::PathBuf>,
    #[arg(long, global = true, value_name = "GRADLE_PATH")]
    module: Option<String>,
    #[arg(long, global = true, value_name = "NAME")]
    variant: Option<String>,
    #[arg(long, global = true, value_name = "SERIAL_OR_SELECTOR")]
    device: Option<String>,
    #[arg(long, global = true, value_name = "NAME")]
    profile: Option<String>,
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    #[arg(long, global = true, value_name = "ARG", action = clap::ArgAction::Append)]
    gradle_arg: Vec<String>,
    #[arg(long, global = true)]
    no_color: bool,
    #[arg(long, global = true)]
    ascii: bool,
    #[arg(long, global = true, value_name = "PATH")]
    debug_log: Option<std::path::PathBuf>,
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<std::path::PathBuf>,
    #[arg(long, global = true)]
    yes: bool,
    #[command(subcommand)]
    command: Option<CliCommand>,
}

impl fmt::Debug for Cli {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cli")
            .field("has_project", &self.project.is_some())
            .field("has_module", &self.module.is_some())
            .field("has_variant", &self.variant.is_some())
            .field("has_device", &self.device.is_some())
            .field("has_profile", &self.profile.is_some())
            .field("format", &self.format)
            .field("gradle_argument_count", &self.gradle_arg.len())
            .field("no_color", &self.no_color)
            .field("ascii", &self.ascii)
            .field("has_debug_log", &self.debug_log.is_some())
            .field("has_config", &self.config.is_some())
            .field("yes", &self.yes)
            .field("has_command", &self.command.is_some())
            .finish()
    }
}

impl Cli {
    #[must_use]
    pub fn shell_options(&self) -> Option<ShellOptions> {
        self.command.is_none().then_some(ShellOptions {
            no_color: self.no_color,
            ascii: self.ascii,
        })
    }

    #[must_use]
    pub fn debug_log_path(&self) -> Option<&std::path::Path> {
        self.debug_log.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
#[value(rename_all = "lower")]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Jsonl,
}

#[derive(Subcommand)]
enum CliCommand {
    Init,
    Doctor,
    Project(ProjectArgs),
    Modules(ListArgs),
    Variants(ListArgs),
    Devices(ListArgs),
    Emulators(ListArgs),
    Build,
    Install,
    Launch,
    Run,
    Rerun,
    Reinstall,
    CleanReinstall,
    Stop,
    Test,
    Logs,
    Gradle(GradleArgs),
    Emulator(EmulatorArgs),
    Command(CustomCommandArgs),
    Version,
}

#[derive(Args)]
struct ProjectArgs {
    #[command(subcommand)]
    command: ProjectCommand,
}

#[derive(Subcommand)]
enum ProjectCommand {
    Inspect,
}

#[derive(Args)]
struct ListArgs {
    #[command(subcommand)]
    command: ListCommand,
}

#[derive(Subcommand)]
enum ListCommand {
    List,
}

#[derive(Args)]
struct GradleArgs {
    #[arg(required = true, num_args = 1.., value_name = "TASK")]
    tasks: Vec<String>,
}

#[derive(Args)]
struct EmulatorArgs {
    #[command(subcommand)]
    command: EmulatorCommand,
}

#[derive(Subcommand)]
enum EmulatorCommand {
    Start { name: String },
    ColdBoot { name: String },
    Wipe { name: String },
    Stop { name: String },
}

#[derive(Args)]
struct CustomCommandArgs {
    #[command(subcommand)]
    command: CustomCommand,
}

#[derive(Subcommand)]
enum CustomCommand {
    Run { name: String },
}

impl CliCommand {
    fn output_kind(&self) -> OutputKind {
        match self {
            Self::Init
            | Self::Doctor
            | Self::Project(_)
            | Self::Modules(_)
            | Self::Variants(_)
            | Self::Devices(_)
            | Self::Emulators(_)
            | Self::Version => OutputKind::Snapshot,
            Self::Build
            | Self::Install
            | Self::Launch
            | Self::Run
            | Self::Rerun
            | Self::Reinstall
            | Self::CleanReinstall
            | Self::Stop
            | Self::Test
            | Self::Logs
            | Self::Gradle(_)
            | Self::Emulator(_)
            | Self::Command(_) => OutputKind::Streaming,
        }
    }

    fn allows_yes(&self) -> bool {
        matches!(
            self,
            Self::CleanReinstall
                | Self::Emulator(EmulatorArgs {
                    command: EmulatorCommand::Wipe { .. }
                })
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputKind {
    Snapshot,
    Streaming,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalCapabilities {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
}

impl TerminalCapabilities {
    #[must_use]
    pub fn detect() -> Self {
        Self {
            stdin: io::stdin().is_terminal(),
            stdout: io::stdout().is_terminal(),
            stderr: io::stderr().is_terminal(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DexdeckExitCode {
    Success = 0,
    OperationFailed = 1,
    InvalidUsage = 2,
    ProjectUnavailable = 3,
    ToolMissing = 4,
    DeviceError = 5,
    Cancelled = 6,
    IncompatibleData = 7,
    Internal = 8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VersionInfo<'a> {
    schema_version: u32,
    product: &'a str,
    version: &'a str,
}

pub fn execute(
    cli: Cli,
    terminal: TerminalCapabilities,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let Some(command) = &cli.command else {
        if !terminal.stdin || !terminal.stdout {
            return write_error(
                stderr,
                "interactive mode requires terminal stdin and stdout; use a subcommand",
            );
        }
        if cli.format != OutputFormat::Human {
            return write_error(stderr, "interactive mode only supports --format human");
        }
        return DexdeckExitCode::Success;
    };

    if let Err(message) = validate(&cli, command) {
        return write_error(stderr, message);
    }

    if matches!(command, CliCommand::Version) {
        return write_version(cli.format, stdout, stderr);
    }

    if matches!(
        command,
        CliCommand::Project(_) | CliCommand::Modules(_) | CliCommand::Variants(_)
    ) {
        return execute_project_command(&cli, command, stdout, stderr);
    }

    // Feature handlers are wired in their implementation phases. Parsing and validation are
    // deliberately complete here so scripts can rely on one stable command grammar.
    DexdeckExitCode::Success
}

fn execute_project_command(
    cli: &Cli,
    command: &CliCommand,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let start = match cli.project.clone().map_or_else(std::env::current_dir, Ok) {
        Ok(path) => path,
        Err(error) => {
            return write_operation_error(
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                &format!("cannot resolve the current directory: {error}"),
            );
        }
    };
    let discovery = match discover_project(&start, cli.project.is_some()) {
        Ok(discovery) => discovery,
        Err(error) => {
            return write_operation_error(
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                &error.to_string(),
            );
        }
    };
    let cached = ProjectIdentity::from_path(&discovery.root)
        .ok()
        .and_then(|identity| {
            StoragePaths::discover()
                .ok()
                .map(|paths| paths.project(&identity))
        })
        .and_then(|paths| load_model(&paths.model).ok().flatten());
    let (mut model, freshness) = cached.map_or_else(
        || (ProjectModel::empty(discovery.root.clone()), "provisional"),
        |model| (model, "current"),
    );
    model
        .modules
        .sort_by(|left, right| left.path.cmp(&right.path));
    for module in &mut model.modules {
        module
            .variants
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    let degraded = model
        .build
        .agp_version
        .as_deref()
        .and_then(|version| select_adapter(Some(version)).ok())
        .is_some_and(|adapter| adapter == AdapterKind::Degraded);
    let value = match command {
        CliCommand::Project(_) => {
            serde_json::json!({ "schemaVersion": CLI_SCHEMA_VERSION, "freshness": freshness, "degraded": degraded, "project": model })
        }
        CliCommand::Modules(_) => {
            serde_json::json!({ "schemaVersion": CLI_SCHEMA_VERSION, "freshness": freshness, "degraded": degraded, "modules": model.modules })
        }
        CliCommand::Variants(_) => {
            let variants = model.modules.iter().flat_map(|module| module.variants.iter().map(move |variant| serde_json::json!({ "module": module.path, "variant": variant }))).collect::<Vec<_>>();
            serde_json::json!({ "schemaVersion": CLI_SCHEMA_VERSION, "freshness": freshness, "degraded": degraded, "variants": variants })
        }
        _ => unreachable!("project command already matched"),
    };
    let result = match cli.format {
        OutputFormat::Json => serde_json::to_writer(&mut *stdout, &value)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stdout)),
        OutputFormat::Human => write_project_human(stdout, command, &value),
        OutputFormat::Jsonl => unreachable!("validated snapshot format"),
    };
    if result.is_err() {
        write_operation_error(
            stderr,
            DexdeckExitCode::Internal,
            "failed to write project output",
        )
    } else {
        DexdeckExitCode::Success
    }
}

fn write_project_human(
    output: &mut dyn Write,
    command: &CliCommand,
    value: &serde_json::Value,
) -> io::Result<()> {
    writeln!(
        output,
        "Freshness: {}",
        value["freshness"].as_str().unwrap_or("unknown")
    )?;
    writeln!(
        output,
        "Degraded: {}",
        value["degraded"].as_bool().unwrap_or(false)
    )?;
    match command {
        CliCommand::Project(_) => writeln!(
            output,
            "Project: {}",
            value["project"]["root"].as_str().unwrap_or("unknown")
        ),
        CliCommand::Modules(_) => {
            for module in value["modules"].as_array().into_iter().flatten() {
                writeln!(
                    output,
                    "{}\t{}",
                    module["path"].as_str().unwrap_or(""),
                    module["kind"].as_str().unwrap_or("")
                )?;
            }
            Ok(())
        }
        CliCommand::Variants(_) => {
            for item in value["variants"].as_array().into_iter().flatten() {
                writeln!(
                    output,
                    "{}\t{}\t{}",
                    item["module"].as_str().unwrap_or(""),
                    item["variant"]["name"].as_str().unwrap_or(""),
                    if item["variant"]["enabled"].as_bool().unwrap_or(false) {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn write_operation_error(
    stderr: &mut dyn Write,
    code: DexdeckExitCode,
    message: &str,
) -> DexdeckExitCode {
    let _ = writeln!(stderr, "dexdeck: {message}");
    code
}

fn validate(cli: &Cli, command: &CliCommand) -> Result<(), &'static str> {
    match (command.output_kind(), cli.format) {
        (OutputKind::Snapshot, OutputFormat::Jsonl) => {
            return Err("snapshot commands support --format human or json, not jsonl");
        }
        (OutputKind::Streaming, OutputFormat::Json) => {
            return Err("streaming commands support --format human or jsonl, not json");
        }
        _ => {}
    }
    if cli.yes && !command.allows_yes() {
        return Err("--yes is only valid for clean-reinstall and emulator wipe");
    }
    Ok(())
}

fn write_version(
    format: OutputFormat,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let version = env!("CARGO_PKG_VERSION");
    let result = match format {
        OutputFormat::Human => writeln!(stdout, "DexDeck {version}"),
        OutputFormat::Json => serde_json::to_writer(
            &mut *stdout,
            &VersionInfo {
                schema_version: CLI_SCHEMA_VERSION,
                product: "DexDeck",
                version,
            },
        )
        .map_err(io::Error::other)
        .and_then(|()| writeln!(stdout)),
        OutputFormat::Jsonl => unreachable!("validated before version output"),
    };
    if result.is_err() {
        let _ = writeln!(stderr, "dexdeck: failed to write output");
        DexdeckExitCode::Internal
    } else {
        DexdeckExitCode::Success
    }
}

fn write_error(stderr: &mut dyn Write, message: &str) -> DexdeckExitCode {
    let _ = writeln!(stderr, "dexdeck: {message}");
    DexdeckExitCode::InvalidUsage
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    fn terminals() -> TerminalCapabilities {
        TerminalCapabilities {
            stdin: true,
            stdout: true,
            stderr: true,
        }
    }

    #[test]
    fn command_definition_is_internally_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_complete_nested_surface() {
        let cases = [
            vec!["dexdeck", "project", "inspect"],
            vec!["dexdeck", "modules", "list"],
            vec!["dexdeck", "emulator", "cold-boot", "pixel"],
            vec!["dexdeck", "command", "run", "backend"],
            vec!["dexdeck", "gradle", "assembleDebug", "lint"],
            vec!["dexdeck", "clean-reinstall", "--yes"],
        ];
        for arguments in cases {
            assert!(Cli::try_parse_from(arguments).is_ok());
        }
    }

    #[test]
    fn validates_snapshot_and_streaming_formats() {
        let snapshot = Cli::try_parse_from(["dexdeck", "project", "inspect", "--format", "jsonl"]);
        let streaming = Cli::try_parse_from(["dexdeck", "logs", "--format", "json"]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        assert_eq!(
            execute(
                snapshot.unwrap_or_else(|error| error.exit()),
                terminals(),
                &mut stdout,
                &mut stderr
            ),
            DexdeckExitCode::InvalidUsage
        );
        stderr.clear();
        assert_eq!(
            execute(
                streaming.unwrap_or_else(|error| error.exit()),
                terminals(),
                &mut stdout,
                &mut stderr
            ),
            DexdeckExitCode::InvalidUsage
        );
    }

    #[test]
    fn refuses_tui_without_a_terminal() {
        let cli = Cli::parse_from(["dexdeck"]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = execute(
            cli,
            TerminalCapabilities {
                stdin: false,
                stdout: false,
                stderr: false,
            },
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(code, DexdeckExitCode::InvalidUsage);
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());
    }

    #[test]
    fn emits_schema_versioned_version_json_on_stdout() {
        let cli = Cli::parse_from(["dexdeck", "version", "--format", "json"]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            execute(cli, terminals(), &mut stdout, &mut stderr),
            DexdeckExitCode::Success
        );
        let value: serde_json::Value =
            serde_json::from_slice(&stdout).unwrap_or_else(|error| panic!("invalid JSON: {error}"));
        assert_eq!(value["schemaVersion"], CLI_SCHEMA_VERSION);
        assert_eq!(value["product"], "DexDeck");
        assert!(stderr.is_empty());
    }

    #[test]
    fn emits_project_freshness_in_deterministic_json() {
        let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(
            temp.path().join("settings.gradle.kts"),
            "rootProject.name = \"fixture\"\n",
        )
        .unwrap_or_else(|error| panic!("fixture: {error}"));
        let cli = Cli::parse_from([
            "dexdeck",
            "--project",
            temp.path()
                .to_str()
                .unwrap_or_else(|| panic!("non-UTF8 temp path")),
            "project",
            "inspect",
            "--format",
            "json",
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            execute(cli, terminals(), &mut stdout, &mut stderr),
            DexdeckExitCode::Success
        );
        let value: serde_json::Value =
            serde_json::from_slice(&stdout).unwrap_or_else(|error| panic!("JSON: {error}"));
        assert_eq!(value["schemaVersion"], CLI_SCHEMA_VERSION);
        assert_eq!(value["freshness"], "provisional");
        assert_eq!(value["degraded"], false);
        assert!(stderr.is_empty());
    }
}
