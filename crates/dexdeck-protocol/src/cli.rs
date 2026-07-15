use serde::{Deserialize, Serialize};

use crate::{
    CLI_SCHEMA_VERSION, Diagnostic, JobId, JobKind, JobRecord, LogRecord, ProjectModel,
    TestRunResult,
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
    pub project: ProjectModel,
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
    JobFinished {
        job: JobRecord,
    },
    Error {
        error: OperationError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationError {
    pub code: String,
    pub category: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<String>,
}
