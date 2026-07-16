use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    time::Duration,
};

use dexdeck_core::{CommandSpec, ProcessSupervisor};
use dexdeck_protocol::{AndroidAvd, AndroidDevice, DeviceState};
use tokio_util::sync::CancellationToken;

use crate::{AdbClient, AdbError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmulatorLaunch {
    pub cold_boot: bool,
    pub wipe_data: bool,
    pub wipe_confirmed: bool,
}

#[derive(Clone, Debug)]
pub struct EmulatorService {
    emulator: PathBuf,
    sdk_root: PathBuf,
    avd_home: PathBuf,
    adb: Arc<AdbClient>,
    supervisor: ProcessSupervisor,
    boot_timeout: Duration,
    poll_interval: Duration,
}

impl EmulatorService {
    #[must_use]
    pub fn new(
        emulator: PathBuf,
        sdk_root: PathBuf,
        avd_home: PathBuf,
        adb: Arc<AdbClient>,
        supervisor: ProcessSupervisor,
    ) -> Self {
        Self {
            emulator,
            sdk_root,
            avd_home,
            adb,
            supervisor,
            boot_timeout: Duration::from_secs(180),
            poll_interval: Duration::from_secs(1),
        }
    }

    #[must_use]
    pub const fn with_boot_timing(mut self, timeout: Duration, poll: Duration) -> Self {
        self.boot_timeout = timeout;
        self.poll_interval = poll;
        self
    }

    pub async fn list(&self, devices: &[AndroidDevice]) -> Result<Vec<AndroidAvd>, EmulatorError> {
        let spec = CommandSpec::new(&self.emulator, &self.sdk_root)?.arg("-list-avds");
        let result = self
            .supervisor
            .run(&spec, CancellationToken::new(), CancellationToken::new())
            .await?;
        if result.exit_code != Some(0) {
            return Err(EmulatorError::Command(result.stderr.text_lossy()));
        }
        let mut values = result
            .stdout
            .text_lossy()
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|name| self.inspect_with_devices(name, devices))
            .collect::<Result<Vec<_>, _>>()?;
        values.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(values)
    }

    pub fn inspect(&self, name: &str) -> Result<AndroidAvd, EmulatorError> {
        self.inspect_with_devices(name, &[])
    }

    fn inspect_with_devices(
        &self,
        name: &str,
        devices: &[AndroidDevice],
    ) -> Result<AndroidAvd, EmulatorError> {
        validate_name(name)?;
        let directory = self.avd_home.join(format!("{name}.avd"));
        let config_path = directory.join("config.ini");
        if !config_path.is_file() {
            return Err(EmulatorError::UnknownAvd(name.into()));
        }
        let properties = read_properties(&config_path)?;
        let running_serial = devices
            .iter()
            .find(|device| device.avd_name.as_deref() == Some(name))
            .map(|device| device.serial.clone());
        Ok(AndroidAvd {
            name: name.into(),
            path: Some(directory),
            device: properties.get("hw.device.name").cloned(),
            target: properties.get("target").cloned(),
            abi: properties
                .get("abi.type")
                .or_else(|| properties.get("hw.cpu.arch"))
                .cloned(),
            running_serial,
        })
    }

    pub async fn start(
        &self,
        name: &str,
        launch: EmulatorLaunch,
        devices: &[AndroidDevice],
        cancel: CancellationToken,
    ) -> Result<String, EmulatorError> {
        let avd = self.inspect_with_devices(name, devices)?;
        if let Some(serial) = avd.running_serial {
            return Err(EmulatorError::AlreadyRunning {
                name: name.into(),
                serial,
            });
        }
        if launch.wipe_data && !launch.wipe_confirmed {
            return Err(EmulatorError::ConfirmationRequired);
        }
        let mut command = Command::new(&self.emulator);
        command.arg("-avd").arg(name);
        if launch.cold_boot {
            command.arg("-no-snapshot-load");
        }
        if launch.wipe_data {
            command.arg("-wipe-data");
        }
        command
            .current_dir(&self.sdk_root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_detached(&mut command);
        command.spawn().map_err(|source| EmulatorError::Spawn {
            path: self.emulator.clone(),
            source,
        })?;

        let deadline = tokio::time::Instant::now() + self.boot_timeout;
        loop {
            if cancel.is_cancelled() {
                return Err(EmulatorError::MonitoringCancelled);
            }
            let devices = self.adb.devices().await.unwrap_or_default();
            if let Some(device) = devices.iter().find(|device| {
                device.avd_name.as_deref() == Some(name) && device.state == DeviceState::Online
            }) && self
                .adb
                .shell(&device.serial, &["getprop", "sys.boot_completed"])
                .await
                .is_ok_and(|value| value.trim() == "1")
            {
                return Ok(device.serial.clone());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(EmulatorError::BootTimeout(name.into()));
            }
            tokio::select! {
                () = cancel.cancelled() => return Err(EmulatorError::MonitoringCancelled),
                () = tokio::time::sleep(self.poll_interval) => {}
            }
        }
    }

    pub async fn stop(&self, name: &str, devices: &[AndroidDevice]) -> Result<(), EmulatorError> {
        let matches = devices
            .iter()
            .filter(|device| device.avd_name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        let [device] = matches.as_slice() else {
            return if matches.is_empty() {
                Err(EmulatorError::NotRunning(name.into()))
            } else {
                Err(EmulatorError::DuplicateRunning(name.into()))
            };
        };
        self.adb.kill_emulator(&device.serial).await?;
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<(), EmulatorError> {
    if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
        return Err(EmulatorError::InvalidName(name.into()));
    }
    Ok(())
}

fn read_properties(path: &Path) -> Result<BTreeMap<String, String>, EmulatorError> {
    let source = fs::read_to_string(path).map_err(|source| EmulatorError::Config {
        path: path.into(),
        source,
    })?;
    Ok(source
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().into(), value.trim().into()))
        .collect())
}

#[cfg(windows)]
fn configure_detached(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(windows))]
fn configure_detached(_command: &mut Command) {}

#[derive(Debug, thiserror::Error)]
pub enum EmulatorError {
    #[error(transparent)]
    Process(#[from] dexdeck_core::ProcessError),
    #[error(transparent)]
    Adb(#[from] AdbError),
    #[error("emulator command failed: {0}")]
    Command(String),
    #[error("invalid AVD name {0:?}")]
    InvalidName(String),
    #[error("AVD {0:?} does not exist")]
    UnknownAvd(String),
    #[error("failed to read AVD configuration {path:?}: {source}")]
    Config {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("AVD {name:?} is already running as {serial}")]
    AlreadyRunning { name: String, serial: String },
    #[error("AVD wipe requires confirmation")]
    ConfirmationRequired,
    #[error("failed to launch emulator {path:?}: {source}")]
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("boot monitoring was cancelled; the emulator is still running")]
    MonitoringCancelled,
    #[error("AVD {0:?} is still running but did not boot before the timeout")]
    BootTimeout(String),
    #[error("AVD {0:?} is not running")]
    NotRunning(String),
    #[error("multiple running emulators report AVD {0:?}")]
    DuplicateRunning(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_reads_existing_avd_and_rejects_path_escape()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let avd = temp.path().join("Pixel.avd");
        fs::create_dir(&avd)?;
        fs::write(
            avd.join("config.ini"),
            "hw.device.name=pixel_8\ntarget=android-35\nabi.type=x86_64\n",
        )?;
        let adb = Arc::new(AdbClient::new(
            "adb".into(),
            temp.path().into(),
            ProcessSupervisor::default(),
        ));
        let service = EmulatorService::new(
            "emulator".into(),
            temp.path().into(),
            temp.path().into(),
            adb,
            ProcessSupervisor::default(),
        );
        let value = service.inspect("Pixel")?;
        assert_eq!(value.target.as_deref(), Some("android-35"));
        assert!(matches!(
            service.inspect("../Pixel"),
            Err(EmulatorError::InvalidName(_))
        ));
        Ok(())
    }

    #[test]
    fn wipe_requires_explicit_confirmation_before_spawn() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let avd = temp.path().join("Pixel.avd");
        fs::create_dir(&avd)?;
        fs::write(avd.join("config.ini"), "")?;
        let adb = Arc::new(AdbClient::new(
            "adb".into(),
            temp.path().into(),
            ProcessSupervisor::default(),
        ));
        let service = EmulatorService::new(
            "missing-emulator".into(),
            temp.path().into(),
            temp.path().into(),
            adb,
            ProcessSupervisor::default(),
        );
        let runtime = tokio::runtime::Runtime::new()?;
        let result = runtime.block_on(service.start(
            "Pixel",
            EmulatorLaunch {
                wipe_data: true,
                ..EmulatorLaunch::default()
            },
            &[],
            CancellationToken::new(),
        ));
        assert!(matches!(result, Err(EmulatorError::ConfirmationRequired)));
        Ok(())
    }
}
