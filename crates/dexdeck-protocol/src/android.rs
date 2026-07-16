use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SdkSource {
    Cli,
    Configuration,
    Model,
    LocalProperties,
    AndroidSdkRoot,
    AndroidHome,
    PlatformDefault,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidTools {
    pub sdk_root: PathBuf,
    pub source: SdkSource,
    pub adb: PathBuf,
    pub emulator: PathBuf,
    pub sdkmanager: PathBuf,
    pub avdmanager: PathBuf,
    pub java: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DoctorStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub code: String,
    pub status: DoctorStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSnapshot {
    pub tools: Option<AndroidTools>,
    pub checks: Vec<DoctorCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceState {
    Online,
    Offline,
    Unauthorized,
    NoPermissions,
    Bootloader,
    Recovery,
    Sideload,
    Unknown(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransportType {
    Usb,
    Local,
    Emulator,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidDevice {
    pub serial: String,
    pub state: DeviceState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_level: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_id: Option<u64>,
    pub transport_type: TransportType,
    pub emulator: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avd_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DevicesSnapshot {
    pub devices: Vec<AndroidDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_serial: Option<String>,
}
