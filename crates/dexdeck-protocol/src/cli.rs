use serde::{Deserialize, Serialize};

use crate::{
    AndroidModule, CLI_SCHEMA_VERSION, Diagnostic, JobId, JobKind, JobRecord, JobState, LogRecord,
    ProjectModel, TestRunResult, Variant,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliEnvelope<T> {
    pub schema_version: u32,
    #[serde(flatten)]
    pub payload: T,
}

impl<T> CliEnvelope<T> {
    #[must_use]
    pub const fn new(payload: T) -> Self {
        Self {
            schema_version: CLI_SCHEMA_VERSION,
            payload,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshot {
    pub freshness: ModelFreshness,
    pub support: ProjectSupport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<DegradedReason>,
    pub project: ProjectModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelFreshness {
    Provisional,
    Current,
    Stale,
    Refreshing,
    Degraded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectSupport {
    Full,
    Degraded,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "reason",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DegradedReason {
    UnsupportedAgp { detected: String, supported: String },
    IncompatibleProtocol { expected: u32, found: u32 },
    ApiUnavailable { api: String },
    MissingWrapper,
    ConfigurationFailed { message: String },
    BridgeFailed { code: String, message: String },
    CacheInvalid { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModulesSnapshot {
    pub freshness: ModelFreshness,
    pub support: ProjectSupport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<DegradedReason>,
    pub modules: Vec<AndroidModule>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleVariant {
    pub module: String,
    pub variant: Variant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantsSnapshot {
    pub freshness: ModelFreshness,
    pub support: ProjectSupport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<DegradedReason>,
    pub variants: Vec<ModuleVariant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CliEvent {
    JobStarted {
        job_id: JobId,
        kind: JobKind,
    },
    JobProgress {
        job_id: JobId,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        completed: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    WorkflowStep {
        job_id: JobId,
        step: WorkflowStepData,
    },
    Output {
        job_id: JobId,
        stream: String,
        text: String,
    },
    Diagnostic {
        job_id: JobId,
        diagnostic: Diagnostic,
    },
    TestResult {
        job_id: JobId,
        result: TestRunResult,
    },
    Log {
        record: LogRecord,
    },
    LogStatus {
        status: LogStatusData,
    },
    JobFinished {
        job: JobRecord,
    },
    Error {
        error: OperationError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepData {
    pub index: u32,
    pub total: u32,
    pub name: String,
    pub state: JobState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogStatusData {
    pub connected: bool,
    pub reconnects: u64,
    pub batches_dropped: u64,
    pub records_dropped: u64,
    pub tracked_processes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationError {
    pub code: ErrorCode,
    pub category: ErrorCategory,
    pub message: String,
    pub context: OperationContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "configuration.invalid")]
    InvalidConfiguration,
    #[serde(rename = "project.not_found")]
    ProjectNotFound,
    #[serde(rename = "project.unsupported")]
    UnsupportedProject,
    #[serde(rename = "bridge.failed")]
    BridgeFailed,
    #[serde(rename = "bridge.protocol")]
    BridgeProtocol,
    #[serde(rename = "gradle.failed")]
    GradleFailed,
    #[serde(rename = "tool.missing")]
    ToolMissing,
    #[serde(rename = "sdk.missing")]
    SdkMissing,
    #[serde(rename = "sdk.invalid")]
    SdkInvalid,
    #[serde(rename = "device.unavailable")]
    DeviceUnavailable,
    #[serde(rename = "device.unauthorized")]
    DeviceUnauthorized,
    #[serde(rename = "device.ambiguous")]
    DeviceAmbiguous,
    #[serde(rename = "emulator.failed")]
    EmulatorFailed,
    #[serde(rename = "emulator.boot_timeout")]
    EmulatorBootTimeout,
    #[serde(rename = "artifact.missing")]
    ArtifactMissing,
    #[serde(rename = "artifact.invalid")]
    ArtifactInvalid,
    #[serde(rename = "confirmation.required")]
    ConfirmationRequired,
    #[serde(rename = "trust.required")]
    TrustRequired,
    #[serde(rename = "permission.denied")]
    PermissionDenied,
    #[serde(rename = "cache.invalid")]
    CacheInvalid,
    #[serde(rename = "operation.cancelled")]
    Cancelled,
    #[serde(rename = "logcat.unavailable")]
    LogcatUnavailable,
    #[serde(rename = "logcat.capture_failed")]
    LogcatCaptureFailed,
    #[serde(rename = "logcat.invalid_filter")]
    LogcatInvalidFilter,
    #[serde(rename = "logcat.export_failed")]
    LogcatExportFailed,
    #[serde(rename = "logcat.recording_failed")]
    LogcatRecordingFailed,
    #[serde(rename = "internal.error")]
    Internal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCategory {
    Configuration,
    ProjectDetection,
    UnsupportedProject,
    GradleBridge,
    GradleOperation,
    ToolMissing,
    Device,
    Emulator,
    Adb,
    Logcat,
    Test,
    Cache,
    Permission,
    Internal,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationContext {
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    pub previous_model_usable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_output_reference: Option<String>,
}
