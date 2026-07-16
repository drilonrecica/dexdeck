use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use dexdeck_core::{CommandSpec, ProcessSupervisor, TerminationReason};
use dexdeck_protocol::{AndroidDevice, DeviceState, TransportType};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::{Semaphore, watch},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct AdbClient {
    adb: PathBuf,
    working_directory: PathBuf,
    supervisor: ProcessSupervisor,
    timeout: Duration,
}

impl AdbClient {
    #[must_use]
    pub fn new(adb: PathBuf, working_directory: PathBuf, supervisor: ProcessSupervisor) -> Self {
        Self {
            adb,
            working_directory,
            supervisor,
            timeout: Duration::from_secs(5),
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub async fn devices(&self) -> Result<Vec<AndroidDevice>, AdbError> {
        Ok(parse_device_list(&self.run(["devices", "-l"]).await?))
    }

    pub async fn restart_server(&self) -> Result<(), AdbError> {
        self.run(["kill-server"]).await?;
        self.run(["start-server"]).await?;
        Ok(())
    }

    pub async fn shell(&self, serial: &str, arguments: &[&str]) -> Result<String, AdbError> {
        let mut argv = vec!["-s", serial, "shell"];
        argv.extend_from_slice(arguments);
        self.run(argv).await
    }

    pub async fn kill_emulator(&self, serial: &str) -> Result<(), AdbError> {
        self.run(["-s", serial, "emu", "kill"]).await?;
        Ok(())
    }

    async fn run<I, S>(&self, arguments: I) -> Result<String, AdbError>
    where
        I: IntoIterator<Item = S>,
        S: Into<std::ffi::OsString>,
    {
        let spec = CommandSpec::new(&self.adb, &self.working_directory)?.args(arguments);
        let cancel = CancellationToken::new();
        let timeout_cancel = cancel.clone();
        let result = tokio::select! {
            result = self.supervisor.run(&spec, cancel, CancellationToken::new()) => result?,
            () = tokio::time::sleep(self.timeout) => {
                timeout_cancel.cancel();
                return Err(AdbError::Timeout);
            }
        };
        if result.termination != TerminationReason::Exited || result.exit_code != Some(0) {
            return Err(AdbError::Command {
                code: result.exit_code,
                stderr: result.stderr.text_lossy(),
            });
        }
        Ok(result.stdout.text_lossy())
    }
}

#[derive(Clone, Debug)]
pub struct DeviceTracker {
    client: Arc<AdbClient>,
    enrichment_limit: usize,
}

impl DeviceTracker {
    #[must_use]
    pub fn new(client: Arc<AdbClient>) -> Self {
        Self {
            client,
            enrichment_limit: 4,
        }
    }

    #[must_use]
    pub const fn with_enrichment_limit(mut self, limit: usize) -> Self {
        self.enrichment_limit = limit;
        self
    }

    pub async fn track(
        &self,
        updates: watch::Sender<Vec<AndroidDevice>>,
        cancel: CancellationToken,
    ) -> Result<(), AdbError> {
        if self.enrichment_limit == 0 {
            return Err(AdbError::InvalidConcurrency);
        }
        let mut delay = Duration::from_millis(250);
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let result = self.track_once(&updates, &cancel).await;
            if cancel.is_cancelled() {
                return Ok(());
            }
            if matches!(result, Err(AdbError::Spawn { .. })) {
                return result;
            }
            tokio::select! {
                () = cancel.cancelled() => return Ok(()),
                () = tokio::time::sleep(delay) => {}
            }
            delay = (delay * 2).min(Duration::from_secs(10));
        }
    }

    async fn track_once(
        &self,
        updates: &watch::Sender<Vec<AndroidDevice>>,
        cancel: &CancellationToken,
    ) -> Result<(), AdbError> {
        let mut child = Command::new(&self.client.adb)
            .args(["track-devices", "-l"])
            .current_dir(&self.client.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| AdbError::Spawn {
                path: self.client.adb.clone(),
                source,
            })?;
        let stdout = child.stdout.take().ok_or(AdbError::MissingTrackerOutput)?;
        let mut lines = BufReader::new(stdout).lines();
        let mut block = Vec::new();
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    child.start_kill().map_err(AdbError::Io)?;
                    let _ = child.wait().await;
                    return Ok(());
                }
                line = lines.next_line() => match line.map_err(AdbError::Io)? {
                    Some(line) if line.is_empty() => {
                        updates.send_replace(self.enrich(parse_device_list(&block.join("\n"))).await);
                        block.clear();
                    }
                    Some(line) => block.push(line),
                    None => {
                        let status = child.wait().await.map_err(AdbError::Io)?;
                        return Err(AdbError::TrackerExited(status.code()));
                    }
                }
            }
        }
    }

    async fn enrich(&self, devices: Vec<AndroidDevice>) -> Vec<AndroidDevice> {
        let semaphore = Arc::new(Semaphore::new(self.enrichment_limit));
        let mut tasks = Vec::with_capacity(devices.len());
        for mut device in devices {
            if device.state != DeviceState::Online {
                tasks.push(tokio::spawn(async move { device }));
                continue;
            }
            let semaphore = Arc::clone(&semaphore);
            let client = Arc::clone(&self.client);
            tasks.push(tokio::spawn(async move {
                let Ok(_permit) = semaphore.acquire_owned().await else {
                    return device;
                };
                if let Ok(properties) = client.shell(&device.serial, &["getprop"]).await {
                    apply_properties(&mut device, &properties);
                }
                device
            }));
        }
        let mut enriched = Vec::new();
        for task in tasks {
            if let Ok(device) = task.await {
                enriched.push(device);
            }
        }
        enriched.sort_by(|left, right| left.serial.cmp(&right.serial));
        enriched
    }
}

#[derive(Clone, Debug, Default)]
pub struct DeviceSelector;

impl DeviceSelector {
    pub fn resolve<'a>(
        devices: &'a [AndroidDevice],
        selector: &str,
    ) -> Result<&'a AndroidDevice, AdbError> {
        if let Some(device) = devices.iter().find(|device| device.serial == selector) {
            return Ok(device);
        }
        let selector = selector.to_ascii_lowercase();
        let matches = devices
            .iter()
            .filter(|device| {
                [
                    device.model.as_deref(),
                    device.product.as_deref(),
                    device.device.as_deref(),
                    device.avd_name.as_deref(),
                ]
                .into_iter()
                .flatten()
                .any(|value| value.to_ascii_lowercase() == selector)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [device] => Ok(device),
            [] => Err(AdbError::Unavailable(selector)),
            _ => Err(AdbError::Ambiguous(selector)),
        }
    }

    pub fn restore_last_used<'a>(
        devices: &'a [AndroidDevice],
        serial: &str,
    ) -> Option<&'a AndroidDevice> {
        devices.iter().find(|device| device.serial == serial)
    }
}

#[must_use]
pub fn parse_device_list(value: &str) -> Vec<AndroidDevice> {
    let mut devices = value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("List of devices"))
        .filter_map(parse_device)
        .collect::<Vec<_>>();
    devices.sort_by(|left, right| left.serial.cmp(&right.serial));
    devices
}

fn parse_device(line: &str) -> Option<AndroidDevice> {
    let mut fields = line.split_whitespace();
    let serial = fields.next()?.to_owned();
    let state_value = fields.next().unwrap_or("unknown");
    let state = match state_value {
        "device" => DeviceState::Online,
        "offline" => DeviceState::Offline,
        "unauthorized" => DeviceState::Unauthorized,
        "no" if line.contains("permissions") => DeviceState::NoPermissions,
        "bootloader" => DeviceState::Bootloader,
        "recovery" => DeviceState::Recovery,
        "sideload" => DeviceState::Sideload,
        value => DeviceState::Unknown(value.into()),
    };
    let emulator = serial.starts_with("emulator-");
    let mut device = AndroidDevice {
        transport_type: if emulator {
            TransportType::Emulator
        } else if serial.contains(':') {
            TransportType::Local
        } else {
            TransportType::Usb
        },
        serial,
        state,
        model: None,
        product: None,
        device: None,
        api_level: None,
        transport_id: None,
        emulator,
        avd_name: None,
    };
    for field in fields {
        if let Some((name, value)) = field.split_once(':') {
            match name {
                "model" => device.model = Some(value.replace('_', " ")),
                "product" => device.product = Some(value.into()),
                "device" => device.device = Some(value.into()),
                "transport_id" => device.transport_id = value.parse().ok(),
                _ => {}
            }
        }
    }
    Some(device)
}

fn apply_properties(device: &mut AndroidDevice, value: &str) {
    for line in value.lines() {
        let Some((key, property)) = line
            .strip_prefix('[')
            .and_then(|line| line.split_once("]: ["))
            .and_then(|(key, value)| Some((key, value.strip_suffix(']')?)))
        else {
            continue;
        };
        match key {
            "ro.product.model" => device.model = Some(property.into()),
            "ro.product.name" => device.product = Some(property.into()),
            "ro.product.device" => device.device = Some(property.into()),
            "ro.build.version.sdk" => device.api_level = property.parse().ok(),
            "ro.boot.qemu.avd_name" => device.avd_name = Some(property.into()),
            _ => {}
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AdbError {
    #[error(transparent)]
    Process(#[from] dexdeck_core::ProcessError),
    #[error("ADB command timed out")]
    Timeout,
    #[error("ADB command failed with exit code {code:?}: {stderr}")]
    Command { code: Option<i32>, stderr: String },
    #[error("failed to start ADB tracker {path:?}: {source}")]
    Spawn {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("ADB tracker has no stdout")]
    MissingTrackerOutput,
    #[error("ADB tracker exited unexpectedly with code {0:?}")]
    TrackerExited(Option<i32>),
    #[error("ADB device {0:?} is unavailable")]
    Unavailable(String),
    #[error("ADB device selector {0:?} is ambiguous")]
    Ambiguous(String),
    #[error("device enrichment concurrency must be greater than zero")]
    InvalidConcurrency,
    #[error(transparent)]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_states_metadata_and_selects_exact_serial_first() {
        let devices = parse_device_list(
            "List of devices attached\nserial-1 device product:p model:Pixel_8 device:husky transport_id:4\nserial-2 unauthorized usb:2-1\nserial-3 no permissions (user in plugdev group)\n",
        );
        assert_eq!(devices.len(), 3);
        assert_eq!(devices[0].state, DeviceState::Online);
        assert_eq!(devices[0].model.as_deref(), Some("Pixel 8"));
        assert!(matches!(
            DeviceSelector::resolve(&devices, "serial-1"),
            Ok(device) if device.serial == "serial-1"
        ));
        assert_eq!(devices[2].state, DeviceState::NoPermissions);
    }

    #[test]
    fn ambiguous_alias_fails_and_last_used_never_falls_back() {
        let devices = parse_device_list("one device model:Pixel\ntwo device model:Pixel\n");
        assert!(matches!(
            DeviceSelector::resolve(&devices, "pixel"),
            Err(AdbError::Ambiguous(_))
        ));
        assert!(DeviceSelector::restore_last_used(&devices, "gone").is_none());
    }
}
