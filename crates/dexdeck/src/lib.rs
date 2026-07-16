use std::{
    fmt,
    io::{self, IsTerminal, Write},
    sync::Arc,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use dexdeck_android::{
    AdbClient, ApplicationService, Doctor, EmulatorLaunch, EmulatorService, InstallOptions,
    SdkResolution, SdkResolver,
};
use dexdeck_config::{
    ConfigLayer, ConfigLoader, ConfigSources, GradleConfig, ProjectIdentity, ProjectPaths,
    ResolvedConfig, StoragePaths,
};
use dexdeck_core::{
    CustomCommandService, ProcessSupervisor, RunProfileResolver, RunProfileSelection,
    SecretRedactor, TrustDecision,
};
use dexdeck_gradle::{
    BridgeRunner, FileProjectModelCache, GradleArgumentLayers, GradleRunRequest, GradleTaskRunner,
    ProjectModelService, WatchingModelInputRegistrar, discover_project,
};
use dexdeck_protocol::{
    CLI_SCHEMA_VERSION, CliEnvelope, CliEvent, DevicesSnapshot, EmulatorsSnapshot, ErrorCategory,
    ErrorCode, JobId, JobKind, JobRecord, JobState, ModuleVariant, ModulesSnapshot,
    OperationContext, OperationError, ProjectModel, ProjectSnapshot, VariantsSnapshot,
};
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
    #[arg(long, global = true, value_name = "PATH")]
    sdk: Option<std::path::PathBuf>,
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
            .field("has_sdk", &self.sdk.is_some())
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
    Devices(DevicesArgs),
    Emulators(EmulatorsArgs),
    Build,
    Install(InstallArgs),
    Launch,
    Run(InstallArgs),
    Rerun,
    Reinstall(InstallArgs),
    CleanReinstall(CleanReinstallArgs),
    Stop,
    Uninstall,
    ClearData,
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

#[derive(Args)]
struct DevicesArgs {
    #[command(subcommand)]
    command: DevicesCommand,
}

#[derive(Subcommand)]
enum DevicesCommand {
    List,
    RestartAdb,
}

#[derive(Args)]
struct EmulatorsArgs {
    #[command(subcommand)]
    command: EmulatorsCommand,
}

#[derive(Subcommand)]
enum EmulatorsCommand {
    List,
    Inspect { name: String },
}

#[derive(Args, Clone, Copy, Debug, Default)]
struct InstallArgs {
    #[arg(long)]
    downgrade: bool,
    #[arg(long)]
    grant_all: bool,
}

#[derive(Args, Clone, Copy, Debug, Default)]
struct CleanReinstallArgs {
    #[arg(long)]
    grant_all: bool,
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
            | Self::Devices(DevicesArgs {
                command: DevicesCommand::List,
            })
            | Self::Emulators(_)
            | Self::Version => OutputKind::Snapshot,
            Self::Build
            | Self::Devices(DevicesArgs {
                command: DevicesCommand::RestartAdb,
            })
            | Self::Install(_)
            | Self::Launch
            | Self::Run(_)
            | Self::Rerun
            | Self::Reinstall(_)
            | Self::CleanReinstall(_)
            | Self::Stop
            | Self::Uninstall
            | Self::ClearData
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
            Self::CleanReinstall(_)
                | Self::Uninstall
                | Self::ClearData
                | Self::Install(_)
                | Self::Run(_)
                | Self::Reinstall(_)
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
    if matches!(command, CliCommand::Doctor) {
        return execute_doctor(&cli, stdout, stderr);
    }

    if matches!(
        command,
        CliCommand::Project(_) | CliCommand::Modules(_) | CliCommand::Variants(_)
    ) {
        return execute_project_command(&cli, command, stdout, stderr);
    }
    if matches!(
        command,
        CliCommand::Devices(_)
            | CliCommand::Emulators(_)
            | CliCommand::Emulator(_)
            | CliCommand::Gradle(_)
    ) {
        return execute_android_tool_command(&cli, command, stdout, stderr);
    }
    if matches!(
        command,
        CliCommand::Build
            | CliCommand::Install(_)
            | CliCommand::Launch
            | CliCommand::Run(_)
            | CliCommand::Rerun
            | CliCommand::Reinstall(_)
            | CliCommand::CleanReinstall(_)
            | CliCommand::Stop
            | CliCommand::Uninstall
            | CliCommand::ClearData
    ) {
        return execute_application_command(&cli, command, terminal, stdout, stderr);
    }
    if let CliCommand::Command(arguments) = command {
        return execute_custom_command(&cli, arguments, terminal, stdout, stderr);
    }

    // Feature handlers are wired in their implementation phases. Parsing and validation are
    // deliberately complete here so scripts can rely on one stable command grammar.
    DexdeckExitCode::Success
}

fn execute_android_tool_command(
    cli: &Cli,
    command: &CliCommand,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let project_root = cli.project.clone().or_else(|| std::env::current_dir().ok());
    let configuration = cli.config.as_ref().and_then(|path| {
        ConfigLoader
            .load(&ConfigSources {
                explicit: Some(path.clone()),
                ..ConfigSources::default()
            })
            .ok()
            .and_then(|loaded| loaded.resolved.android.sdk_path)
    });
    let tools = match SdkResolver::default().resolve(&SdkResolution {
        cli: cli.sdk.clone(),
        configuration,
        project_root: project_root.clone(),
        ..SdkResolution::default()
    }) {
        Ok(tools) => tools,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::ToolMissing,
                ErrorCode::SdkMissing,
                &error.to_string(),
            );
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::Internal,
                ErrorCode::Internal,
                &error.to_string(),
            );
        }
    };
    let adb = Arc::new(AdbClient::new(
        tools.adb.clone(),
        tools.sdk_root.clone(),
        ProcessSupervisor::default(),
    ));
    let result: Result<serde_json::Value, String> = runtime.block_on(async {
        match command {
            CliCommand::Devices(DevicesArgs {
                command: DevicesCommand::List,
            }) => {
                let devices = adb
                    .enriched_devices()
                    .await
                    .map_err(|error| error.to_string())?;
                serde_json::to_value(CliEnvelope::new(DevicesSnapshot {
                    devices,
                    selected_serial: None,
                }))
                .map_err(|error| error.to_string())
            }
            CliCommand::Devices(DevicesArgs {
                command: DevicesCommand::RestartAdb,
            }) => {
                adb.restart_server()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(operation_success(JobKind::CustomCommand, "restart-adb"))
            }
            CliCommand::Emulators(arguments) => {
                let devices = adb
                    .enriched_devices()
                    .await
                    .map_err(|error| error.to_string())?;
                let service = EmulatorService::new(
                    tools.emulator.clone(),
                    tools.sdk_root.clone(),
                    avd_home().ok_or_else(|| "cannot resolve Android AVD home".to_owned())?,
                    Arc::clone(&adb),
                    ProcessSupervisor::default(),
                );
                let emulators = match &arguments.command {
                    EmulatorsCommand::List => service
                        .list(&devices)
                        .await
                        .map_err(|error| error.to_string())?,
                    EmulatorsCommand::Inspect { name } => {
                        vec![service.inspect(name).map_err(|error| error.to_string())?]
                    }
                };
                serde_json::to_value(CliEnvelope::new(EmulatorsSnapshot { emulators }))
                    .map_err(|error| error.to_string())
            }
            CliCommand::Emulator(arguments) => {
                let devices = adb
                    .enriched_devices()
                    .await
                    .map_err(|error| error.to_string())?;
                let service = EmulatorService::new(
                    tools.emulator.clone(),
                    tools.sdk_root.clone(),
                    avd_home().ok_or_else(|| "cannot resolve Android AVD home".to_owned())?,
                    Arc::clone(&adb),
                    ProcessSupervisor::default(),
                );
                let name = match &arguments.command {
                    EmulatorCommand::Start { name }
                    | EmulatorCommand::ColdBoot { name }
                    | EmulatorCommand::Wipe { name }
                    | EmulatorCommand::Stop { name } => name,
                };
                match &arguments.command {
                    EmulatorCommand::Start { .. } => {
                        service
                            .start(
                                name,
                                EmulatorLaunch::default(),
                                &devices,
                                tokio_util::sync::CancellationToken::new(),
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    EmulatorCommand::ColdBoot { .. } => {
                        service
                            .start(
                                name,
                                EmulatorLaunch {
                                    cold_boot: true,
                                    ..EmulatorLaunch::default()
                                },
                                &devices,
                                tokio_util::sync::CancellationToken::new(),
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    EmulatorCommand::Wipe { .. } => {
                        service
                            .start(
                                name,
                                EmulatorLaunch {
                                    wipe_data: true,
                                    wipe_confirmed: cli.yes,
                                    cold_boot: false,
                                },
                                &devices,
                                tokio_util::sync::CancellationToken::new(),
                            )
                            .await
                            .map_err(|error| error.to_string())?;
                    }
                    EmulatorCommand::Stop { .. } => service
                        .stop(name, &devices)
                        .await
                        .map_err(|error| error.to_string())?,
                }
                Ok(operation_success(JobKind::Emulator, name))
            }
            CliCommand::Gradle(arguments) => {
                let start =
                    project_root.ok_or_else(|| "cannot resolve project directory".to_owned())?;
                let discovery = discover_project(&start, cli.project.is_some())
                    .map_err(|error| error.to_string())?;
                let runner = GradleTaskRunner::default();
                let result = runner
                    .run(GradleRunRequest {
                        root: discovery.root,
                        tasks: arguments.tasks.clone(),
                        arguments: GradleArgumentLayers {
                            cli: cli.gradle_arg.clone(),
                            ..GradleArgumentLayers::default()
                        },
                        environment: std::collections::BTreeMap::new(),
                        cancel: tokio_util::sync::CancellationToken::new(),
                        force_cancel: tokio_util::sync::CancellationToken::new(),
                        output: None,
                        redactor: SecretRedactor::new(),
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                if result.exit_code != Some(0) {
                    return Err(result.stderr.text_lossy());
                }
                Ok(operation_success(
                    JobKind::Gradle,
                    &arguments.tasks.join(" "),
                ))
            }
            _ => Err("unsupported Android command".into()),
        }
    });
    let value = match result {
        Ok(value) => value,
        Err(error) => {
            let (exit, code) = if matches!(command, CliCommand::Gradle(_)) {
                (DexdeckExitCode::OperationFailed, ErrorCode::GradleFailed)
            } else {
                (DexdeckExitCode::DeviceError, ErrorCode::EmulatorFailed)
            };
            return write_android_error(cli, stderr, exit, code, &error);
        }
    };
    let write_result = match cli.format {
        OutputFormat::Json | OutputFormat::Jsonl => serde_json::to_writer(&mut *stdout, &value)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stdout)),
        OutputFormat::Human if command.output_kind() == OutputKind::Snapshot => {
            write_snapshot_human(stdout, &value)
        }
        OutputFormat::Human => writeln!(stderr, "completed"),
    };
    if write_result.is_err() {
        DexdeckExitCode::Internal
    } else {
        DexdeckExitCode::Success
    }
}

struct OperationProject {
    root: std::path::PathBuf,
    model: ProjectModel,
    config: ResolvedConfig,
    paths: ProjectPaths,
}

fn load_operation_project(cli: &Cli) -> Result<OperationProject, String> {
    let start = cli
        .project
        .clone()
        .map_or_else(std::env::current_dir, Ok)
        .map_err(|error| error.to_string())?;
    let storage = StoragePaths::discover().map_err(|error| error.to_string())?;
    let discovery =
        discover_project(&start, cli.project.is_some()).map_err(|error| error.to_string())?;
    let identity =
        ProjectIdentity::from_path(&discovery.root).map_err(|error| error.to_string())?;
    let paths = storage.project(&identity);
    let loaded = ConfigLoader
        .load(&ConfigSources {
            shared: Some(discovery.root.join(".dexdeck/config.toml")),
            user: Some(paths.user_config.clone()),
            explicit: cli.config.clone(),
            ..ConfigSources::default()
        })
        .map_err(|error| error.to_string())?;
    let service = ProjectModelService::new(
        Arc::new(BridgeRunner::new(
            storage.bridge_cache_root(),
            ProcessSupervisor::default(),
        )),
        Arc::new(FileProjectModelCache::new(storage)),
        Arc::new(WatchingModelInputRegistrar::default()),
    );
    let mut state = service
        .open(&discovery.root, true)
        .map_err(|error| error.to_string())?;
    if state.freshness != dexdeck_protocol::ModelFreshness::Current {
        let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
        state = runtime
            .block_on(service.refresh(
                tokio_util::sync::CancellationToken::new(),
                tokio_util::sync::CancellationToken::new(),
                &SecretRedactor::new(),
            ))
            .or_else(|_| service.state())
            .map_err(|error| error.to_string())?;
    }
    let model = state
        .model
        .ok_or_else(|| "project model is unavailable".to_owned())?;
    Ok(OperationProject {
        root: discovery.root,
        model,
        config: loaded.resolved,
        paths,
    })
}

fn confirm_operation(
    cli: &Cli,
    command: &CliCommand,
    terminal: TerminalCapabilities,
    stderr: &mut dyn Write,
) -> bool {
    if cli.format != OutputFormat::Human
        || !terminal.stdin
        || !terminal.stderr
        || !io::stdin().is_terminal()
    {
        return false;
    }
    let operation = match command {
        CliCommand::CleanReinstall(_) => "clean reinstall",
        CliCommand::Uninstall => "uninstall",
        CliCommand::ClearData => "clear application data",
        _ => "operate on a release or non-debuggable build",
    };
    let _ = write!(stderr, "Confirm {operation}? [y/N] ");
    let _ = stderr.flush();
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .is_ok_and(|_| matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn execute_application_command(
    cli: &Cli,
    command: &CliCommand,
    terminal: TerminalCapabilities,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let project = match load_operation_project(cli) {
        Ok(project) => project,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                &error,
            );
        }
    };
    let resolver = SdkResolver::default();
    let tools = match resolver.resolve(&SdkResolution {
        cli: cli.sdk.clone(),
        configuration: project.config.android.sdk_path.clone(),
        project_root: Some(project.root.clone()),
        ..SdkResolution::default()
    }) {
        Ok(tools) => tools,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::ToolMissing,
                ErrorCode::SdkMissing,
                &error.to_string(),
            );
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::Internal,
                ErrorCode::Internal,
                &error.to_string(),
            );
        }
    };
    let adb = Arc::new(AdbClient::new(
        tools.adb,
        tools.sdk_root,
        ProcessSupervisor::default(),
    ));
    let devices = match runtime.block_on(adb.enriched_devices()) {
        Ok(devices) => devices,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::DeviceError,
                ErrorCode::DeviceUnavailable,
                &error.to_string(),
            );
        }
    };
    let require_device = !matches!(command, CliCommand::Build);
    let mut redactor = SecretRedactor::new();
    let profile = match RunProfileResolver::resolve(
        &project.model,
        &project.config,
        &devices,
        &RunProfileSelection {
            profile: cli.profile.clone(),
            module: cli.module.clone(),
            variant: cli.variant.clone(),
            device: cli.device.clone(),
            gradle_arguments: cli.gradle_arg.clone(),
            require_device,
        },
        &mut redactor,
    ) {
        Ok(profile) => profile,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::InvalidUsage,
                ErrorCode::InvalidConfiguration,
                &error.to_string(),
            );
        }
    };
    let destructive = matches!(
        command,
        CliCommand::CleanReinstall(_) | CliCommand::Uninstall | CliCommand::ClearData
    );
    if (destructive || profile.release_confirmation_required)
        && !cli.yes
        && !confirm_operation(cli, command, terminal, stderr)
    {
        return write_android_error(
            cli,
            stderr,
            DexdeckExitCode::InvalidUsage,
            ErrorCode::ConfirmationRequired,
            "operation requires interactive confirmation or --yes",
        );
    }
    let kind = match command {
        CliCommand::Build => JobKind::Build,
        CliCommand::Install(_) => JobKind::Install,
        CliCommand::Launch => JobKind::Launch,
        _ => JobKind::Run,
    };
    let result: Result<(), String> = runtime.block_on(async {
        let app = ApplicationService::new(Arc::clone(&adb));
        let gradle = GradleTaskRunner::default();
        let dexdeck_core::ResolvedRunProfile {
            module,
            variant,
            device,
            launch,
            gradle_arguments,
            gradle_properties,
            environment,
            ..
        } = profile;
        let serial = device.as_ref().map(|value| value.serial.as_str());
        let package = variant
            .application_id
            .as_deref()
            .ok_or_else(|| "application ID is missing".to_owned())?;
        let install_options = match command {
            CliCommand::Install(options)
            | CliCommand::Run(options)
            | CliCommand::Reinstall(options) => InstallOptions {
                downgrade: options.downgrade,
                grant_all: options.grant_all,
            },
            CliCommand::CleanReinstall(options) => InstallOptions {
                downgrade: false,
                grant_all: options.grant_all,
            },
            _ => InstallOptions::default(),
        };
        let assemble = || {
            let task = variant
                .tasks
                .assemble
                .clone()
                .ok_or_else(|| "variant assemble task is missing".to_owned());
            async {
                let task = task?;
                let mut env = environment;
                for (name, value) in gradle_properties {
                    env.insert(format!("ORG_GRADLE_PROJECT_{name}"), value);
                }
                let result = gradle
                    .run(GradleRunRequest {
                        root: project.root.clone(),
                        tasks: vec![task],
                        arguments: GradleArgumentLayers {
                            cli: gradle_arguments,
                            ..GradleArgumentLayers::default()
                        },
                        environment: env,
                        cancel: tokio_util::sync::CancellationToken::new(),
                        force_cancel: tokio_util::sync::CancellationToken::new(),
                        output: None,
                        redactor,
                    })
                    .await
                    .map_err(|error| error.to_string())?;
                if result.exit_code == Some(0) {
                    Ok(())
                } else {
                    Err(result.stderr.text_lossy())
                }
            }
        };
        match command {
            CliCommand::Build => assemble().await,
            CliCommand::Install(_) => {
                let apks = match app.discover_apks(&module, &variant) {
                    Ok(apks) => apks,
                    Err(_) => {
                        assemble().await?;
                        app.discover_apks(&module, &variant)
                            .map_err(|error| error.to_string())?
                    }
                };
                app.install(
                    serial.ok_or_else(|| "device is required".to_owned())?,
                    &apks,
                    install_options,
                )
                .await
                .map_err(|error| error.to_string())
            }
            CliCommand::Launch => app
                .launch(
                    serial.ok_or_else(|| "device is required".to_owned())?,
                    &variant,
                    &launch,
                )
                .await
                .map_err(|error| error.to_string()),
            CliCommand::Run(_) | CliCommand::Reinstall(_) | CliCommand::CleanReinstall(_) => {
                assemble().await?;
                let serial = serial.ok_or_else(|| "device is required".to_owned())?;
                if matches!(command, CliCommand::CleanReinstall(_)) {
                    app.uninstall(serial, package)
                        .await
                        .map_err(|error| error.to_string())?;
                }
                let apks = app
                    .discover_apks(&module, &variant)
                    .map_err(|error| error.to_string())?;
                app.install(serial, &apks, install_options)
                    .await
                    .map_err(|error| error.to_string())?;
                app.launch(serial, &variant, &launch)
                    .await
                    .map_err(|error| error.to_string())
            }
            CliCommand::Rerun => {
                let serial = serial.ok_or_else(|| "device is required".to_owned())?;
                app.force_stop(serial, package)
                    .await
                    .map_err(|error| error.to_string())?;
                app.launch(serial, &variant, &launch)
                    .await
                    .map_err(|error| error.to_string())
            }
            CliCommand::Stop => app
                .force_stop(
                    serial.ok_or_else(|| "device is required".to_owned())?,
                    package,
                )
                .await
                .map_err(|error| error.to_string()),
            CliCommand::Uninstall => app
                .uninstall(
                    serial.ok_or_else(|| "device is required".to_owned())?,
                    package,
                )
                .await
                .map_err(|error| error.to_string()),
            CliCommand::ClearData => app
                .clear_data(
                    serial.ok_or_else(|| "device is required".to_owned())?,
                    package,
                )
                .await
                .map_err(|error| error.to_string()),
            _ => Err("unsupported application command".into()),
        }
    });
    match result {
        Ok(()) => {
            let value = operation_success(kind, "application-workflow");
            let output = if cli.format == OutputFormat::Jsonl {
                serde_json::to_writer(&mut *stdout, &value)
                    .map_err(io::Error::other)
                    .and_then(|()| writeln!(stdout))
            } else {
                writeln!(stderr, "completed")
            };
            if output.is_ok() {
                DexdeckExitCode::Success
            } else {
                DexdeckExitCode::Internal
            }
        }
        Err(error) => write_android_error(
            cli,
            stderr,
            DexdeckExitCode::OperationFailed,
            ErrorCode::GradleFailed,
            &error,
        ),
    }
}

fn execute_custom_command(
    cli: &Cli,
    arguments: &CustomCommandArgs,
    terminal: TerminalCapabilities,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> DexdeckExitCode {
    let project = match load_operation_project(cli) {
        Ok(project) => project,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                &error,
            );
        }
    };
    let CustomCommand::Run { name } = &arguments.command;
    let Some(command) = project.config.commands.get(name) else {
        return write_android_error(
            cli,
            stderr,
            DexdeckExitCode::InvalidUsage,
            ErrorCode::InvalidConfiguration,
            &format!("custom command {name:?} does not exist"),
        );
    };
    let service = match CustomCommandService::new(
        &project.root,
        project.paths.trust,
        ProcessSupervisor::default(),
    ) {
        Ok(service) => service,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::InvalidUsage,
                ErrorCode::TrustRequired,
                &error.to_string(),
            );
        }
    };
    let mut redactor = SecretRedactor::new();
    let preview = match service.preview(command, &redactor) {
        Ok(preview) => preview,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::InvalidUsage,
                ErrorCode::TrustRequired,
                &error.to_string(),
            );
        }
    };
    if !preview.already_trusted {
        let _ = writeln!(stderr, "Command: {}", preview.argv.join(" "));
        let _ = writeln!(
            stderr,
            "Working directory: {}",
            preview.working_directory.display()
        );
    }
    let decision = if preview.already_trusted {
        TrustDecision::Once
    } else if terminal.stdin && terminal.stderr && cli.format == OutputFormat::Human {
        let _ = write!(stderr, "Trust [o]nce, trust [p]roject, or [c]ancel? ");
        let _ = stderr.flush();
        let mut answer = String::new();
        match std::io::stdin().read_line(&mut answer) {
            Ok(_)
                if answer.trim().eq_ignore_ascii_case("o")
                    || answer.trim().eq_ignore_ascii_case("once") =>
            {
                TrustDecision::Once
            }
            Ok(_)
                if answer.trim().eq_ignore_ascii_case("p")
                    || answer.trim().eq_ignore_ascii_case("project") =>
            {
                TrustDecision::Project
            }
            _ => TrustDecision::Cancel,
        }
    } else {
        TrustDecision::Once
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            return write_android_error(
                cli,
                stderr,
                DexdeckExitCode::Internal,
                ErrorCode::Internal,
                &error.to_string(),
            );
        }
    };
    match runtime.block_on(service.execute(
        command,
        decision,
        terminal.stdin && terminal.stderr && cli.format == OutputFormat::Human,
        &mut redactor,
        tokio_util::sync::CancellationToken::new(),
        tokio_util::sync::CancellationToken::new(),
    )) {
        Ok(result) if result.exit_code == Some(0) => {
            let value = operation_success(JobKind::CustomCommand, name);
            let write = if cli.format == OutputFormat::Jsonl {
                serde_json::to_writer(&mut *stdout, &value)
                    .map_err(io::Error::other)
                    .and_then(|()| writeln!(stdout))
            } else {
                writeln!(stderr, "completed")
            };
            if write.is_ok() {
                DexdeckExitCode::Success
            } else {
                DexdeckExitCode::Internal
            }
        }
        Ok(result) => write_android_error(
            cli,
            stderr,
            DexdeckExitCode::OperationFailed,
            ErrorCode::Internal,
            &result.stderr.text_lossy(),
        ),
        Err(error) => write_android_error(
            cli,
            stderr,
            DexdeckExitCode::InvalidUsage,
            ErrorCode::TrustRequired,
            &error.to_string(),
        ),
    }
}

fn operation_success(kind: JobKind, summary: &str) -> serde_json::Value {
    serde_json::to_value(CliEnvelope::new(CliEvent::JobFinished {
        job: JobRecord {
            id: JobId("cli".into()),
            kind,
            state: JobState::Succeeded,
            project_identity: "cli".into(),
            module: None,
            variant: None,
            device: None,
            command_summary: vec![summary.into()],
            started_at: "cli".into(),
            finished_at: Some("cli".into()),
            duration_ms: Some(0),
            exit_code: Some(0),
            diagnostics: vec![],
        },
    }))
    .unwrap_or(serde_json::Value::Null)
}

fn write_snapshot_human(output: &mut dyn Write, value: &serde_json::Value) -> io::Result<()> {
    if let Some(devices) = value.get("devices").and_then(serde_json::Value::as_array) {
        for device in devices {
            writeln!(
                output,
                "{}\t{}\t{}",
                device["serial"].as_str().unwrap_or(""),
                device["state"].as_str().unwrap_or("unknown"),
                device["model"].as_str().unwrap_or("")
            )?;
        }
    } else if let Some(emulators) = value.get("emulators").and_then(serde_json::Value::as_array) {
        for emulator in emulators {
            writeln!(
                output,
                "{}\t{}",
                emulator["name"].as_str().unwrap_or(""),
                emulator["runningSerial"].as_str().unwrap_or("stopped")
            )?;
        }
    }
    Ok(())
}

fn avd_home() -> Option<std::path::PathBuf> {
    std::env::var_os("ANDROID_AVD_HOME")
        .map(Into::into)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".android/avd"))
        })
}

fn write_android_error(
    cli: &Cli,
    stderr: &mut dyn Write,
    exit: DexdeckExitCode,
    code: ErrorCode,
    message: &str,
) -> DexdeckExitCode {
    if matches!(cli.format, OutputFormat::Json | OutputFormat::Jsonl) {
        let event = CliEnvelope::new(CliEvent::Error {
            error: OperationError {
                code,
                category: ErrorCategory::Adb,
                message: message.into(),
                context: OperationContext {
                    operation: "android".into(),
                    previous_model_usable: false,
                    ..OperationContext::default()
                },
                suggested_action: None,
            },
        });
        let _ = serde_json::to_writer(&mut *stderr, &event);
        let _ = writeln!(stderr);
    } else {
        let _ = writeln!(stderr, "dexdeck: {message}");
    }
    exit
}

fn execute_doctor(cli: &Cli, stdout: &mut dyn Write, stderr: &mut dyn Write) -> DexdeckExitCode {
    let project_root = cli.project.clone().or_else(|| std::env::current_dir().ok());
    let configuration = cli.config.as_ref().and_then(|path| {
        ConfigLoader
            .load(&ConfigSources {
                explicit: Some(path.clone()),
                ..ConfigSources::default()
            })
            .ok()
            .and_then(|loaded| loaded.resolved.android.sdk_path)
    });
    let resolver = SdkResolver::default();
    let snapshot = Doctor::inspect(
        &resolver,
        resolver.resolve(&SdkResolution {
            cli: cli.sdk.clone(),
            configuration,
            project_root,
            ..SdkResolution::default()
        }),
    );
    let failed = snapshot
        .checks
        .iter()
        .any(|check| check.status == dexdeck_protocol::DoctorStatus::Error);
    let result = match cli.format {
        OutputFormat::Json => serde_json::to_writer(&mut *stdout, &CliEnvelope::new(snapshot))
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stdout)),
        OutputFormat::Human => {
            let mut result = Ok(());
            for check in &snapshot.checks {
                result =
                    result.and_then(|()| writeln!(stdout, "{:?}: {}", check.status, check.message));
                if let Some(suggestion) = &check.suggestion {
                    result = result.and_then(|()| writeln!(stdout, "  {suggestion}"));
                }
            }
            result
        }
        OutputFormat::Jsonl => unreachable!("validated snapshot format"),
    };
    if result.is_err() {
        let _ = writeln!(stderr, "dexdeck: failed to write doctor output");
        DexdeckExitCode::Internal
    } else if failed {
        DexdeckExitCode::ToolMissing
    } else {
        DexdeckExitCode::Success
    }
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
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                ErrorCategory::ProjectDetection,
                &format!("cannot resolve the current directory: {error}"),
            );
        }
    };
    let storage = match StoragePaths::discover() {
        Ok(storage) => storage,
        Err(error) => {
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::CacheInvalid,
                ErrorCategory::Cache,
                &format!("cannot resolve DexDeck storage: {error}"),
            );
        }
    };
    let discovery = match discover_project(&start, cli.project.is_some()) {
        Ok(discovery) => discovery,
        Err(error) => {
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                ErrorCategory::ProjectDetection,
                &error.to_string(),
            );
        }
    };
    let identity = match ProjectIdentity::from_path(&discovery.root) {
        Ok(identity) => identity,
        Err(error) => {
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                ErrorCategory::ProjectDetection,
                &error.to_string(),
            );
        }
    };
    let project_paths = storage.project(&identity);
    let cli_layer = (!cli.gradle_arg.is_empty()).then(|| ConfigLayer {
        gradle: GradleConfig {
            arguments: Some(cli.gradle_arg.clone()),
        },
        ..ConfigLayer::default()
    });
    if let Err(error) = ConfigLoader.load(&ConfigSources {
        shared: Some(discovery.root.join(".dexdeck/config.toml")),
        user: Some(project_paths.user_config),
        explicit: cli.config.clone(),
        cli: cli_layer,
        ..ConfigSources::default()
    }) {
        return write_project_error(
            cli,
            stderr,
            DexdeckExitCode::InvalidUsage,
            ErrorCode::InvalidConfiguration,
            ErrorCategory::Configuration,
            &error.to_string(),
        );
    }
    let service = ProjectModelService::new(
        Arc::new(BridgeRunner::new(
            storage.bridge_cache_root(),
            ProcessSupervisor::default(),
        )),
        Arc::new(FileProjectModelCache::new(storage)),
        Arc::new(WatchingModelInputRegistrar::default()),
    );
    let mut state = match service.open(&discovery.root, true) {
        Ok(state) => state,
        Err(error) => {
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::ProjectUnavailable,
                ErrorCode::ProjectNotFound,
                ErrorCategory::ProjectDetection,
                &error.to_string(),
            );
        }
    };
    if state.freshness != dexdeck_protocol::ModelFreshness::Current {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                return write_project_error(
                    cli,
                    stderr,
                    DexdeckExitCode::Internal,
                    ErrorCode::Internal,
                    ErrorCategory::Internal,
                    &format!("cannot start project model runtime: {error}"),
                );
            }
        };
        state = match runtime.block_on(service.refresh(
            tokio_util::sync::CancellationToken::new(),
            tokio_util::sync::CancellationToken::new(),
            &SecretRedactor::new(),
        )) {
            Ok(refreshed) => refreshed,
            Err(_) => match service.state() {
                Ok(failed) => failed,
                Err(error) => {
                    return write_project_error(
                        cli,
                        stderr,
                        DexdeckExitCode::Internal,
                        ErrorCode::Internal,
                        ErrorCategory::Internal,
                        &error.to_string(),
                    );
                }
            },
        };
    }
    let mut model = state
        .model
        .unwrap_or_else(|| ProjectModel::empty(start.canonicalize().unwrap_or(start)));
    model
        .modules
        .sort_by(|left, right| left.path.cmp(&right.path));
    for module in &mut model.modules {
        module
            .variants
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    let value = match command {
        CliCommand::Project(_) => serde_json::to_value(CliEnvelope::new(ProjectSnapshot {
            freshness: state.freshness,
            support: state.support,
            degraded_reason: state.degraded_reason.clone(),
            project: model,
        })),
        CliCommand::Modules(_) => serde_json::to_value(CliEnvelope::new(ModulesSnapshot {
            freshness: state.freshness,
            support: state.support,
            degraded_reason: state.degraded_reason.clone(),
            modules: model.modules,
        })),
        CliCommand::Variants(_) => {
            let variants = model
                .modules
                .into_iter()
                .flat_map(|module| {
                    let path = module.path;
                    module
                        .variants
                        .into_iter()
                        .map(move |variant| ModuleVariant {
                            module: path.clone(),
                            variant,
                        })
                })
                .collect();
            serde_json::to_value(CliEnvelope::new(VariantsSnapshot {
                freshness: state.freshness,
                support: state.support,
                degraded_reason: state.degraded_reason,
                variants,
            }))
        }
        _ => unreachable!("project command already matched"),
    };
    let value = match value {
        Ok(value) => value,
        Err(_) => {
            return write_project_error(
                cli,
                stderr,
                DexdeckExitCode::Internal,
                ErrorCode::Internal,
                ErrorCategory::Internal,
                "failed to serialize project output",
            );
        }
    };
    let result = match cli.format {
        OutputFormat::Json => serde_json::to_writer(&mut *stdout, &value)
            .map_err(io::Error::other)
            .and_then(|()| writeln!(stdout)),
        OutputFormat::Human => write_project_human(stdout, command, &value),
        OutputFormat::Jsonl => unreachable!("validated snapshot format"),
    };
    if result.is_err() {
        write_project_error(
            cli,
            stderr,
            DexdeckExitCode::Internal,
            ErrorCode::Internal,
            ErrorCategory::Internal,
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
        "Support: {}",
        value["support"].as_str().unwrap_or("unknown")
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

fn write_project_error(
    cli: &Cli,
    stderr: &mut dyn Write,
    exit: DexdeckExitCode,
    code: ErrorCode,
    category: ErrorCategory,
    message: &str,
) -> DexdeckExitCode {
    if cli.format == OutputFormat::Json {
        let report = CliEnvelope::new(OperationError {
            code,
            category,
            message: message.to_owned(),
            context: OperationContext {
                operation: "project-model".into(),
                previous_model_usable: false,
                ..OperationContext::default()
            },
            suggested_action: None,
        });
        if serde_json::to_writer(&mut *stderr, &report).is_ok() {
            let _ = writeln!(stderr);
        }
    } else {
        let _ = writeln!(stderr, "dexdeck: {message}");
    }
    exit
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
        return Err(
            "--yes is only valid for destructive actions and release-capable install workflows",
        );
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
            vec!["dexdeck", "devices", "restart-adb"],
            vec!["dexdeck", "emulators", "inspect", "Pixel"],
            vec!["dexdeck", "run", "--downgrade", "--grant-all", "--yes"],
            vec!["dexdeck", "uninstall", "--yes"],
            vec!["dexdeck", "clear-data", "--yes"],
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
        std::fs::write(
            temp.path().join("build.gradle.kts"),
            "plugins { id(\"com.android.application\") }\n",
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
        assert_eq!(value["freshness"], "degraded");
        assert_eq!(value["support"], "degraded");
        assert_eq!(value["degradedReason"]["reason"], "missingWrapper");
        assert!(stderr.is_empty());
    }

    #[test]
    fn rejects_invalid_explicit_configuration_before_modeling() {
        let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        std::fs::write(
            temp.path().join("settings.gradle"),
            "rootProject.name = 'fixture'\n",
        )
        .unwrap_or_else(|error| panic!("fixture: {error}"));
        std::fs::write(
            temp.path().join("build.gradle"),
            "plugins { id 'com.android.application' }\n",
        )
        .unwrap_or_else(|error| panic!("fixture: {error}"));
        let config = temp.path().join("invalid.toml");
        std::fs::write(&config, "schema_version = 1\n[logcat]\nbuffer_mib = 1\n")
            .unwrap_or_else(|error| panic!("config: {error}"));
        let cli = Cli::parse_from([
            "dexdeck",
            "--project",
            temp.path()
                .to_str()
                .unwrap_or_else(|| panic!("non-UTF8 temp path")),
            "--config",
            config
                .to_str()
                .unwrap_or_else(|| panic!("non-UTF8 config path")),
            "project",
            "inspect",
            "--format",
            "json",
        ]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        assert_eq!(
            execute(cli, terminals(), &mut stdout, &mut stderr),
            DexdeckExitCode::InvalidUsage
        );
        let value: serde_json::Value = serde_json::from_slice(&stderr)
            .unwrap_or_else(|error| panic!("structured error: {error}"));
        assert_eq!(value["code"], "configuration.invalid");
        assert!(stdout.is_empty());
    }
}
