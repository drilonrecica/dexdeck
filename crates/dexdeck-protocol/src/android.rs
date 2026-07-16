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
