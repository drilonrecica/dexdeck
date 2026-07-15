use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AndroidModule, BRIDGE_PROTOCOL_VERSION, BuildInfo, Diagnostic, FlavorDimension, GradleTask,
    ProductFlavor, Variant,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEnvelope {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub payload: BridgePayload,
}

impl BridgeEnvelope {
    #[must_use]
    pub fn new(payload: BridgePayload) -> Self {
        Self {
            protocol_version: BRIDGE_PROTOCOL_VERSION,
            payload,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BridgePayload {
    Build {
        build: BuildInfo,
    },
    Module {
        module: AndroidModule,
    },
    Dimension {
        module: String,
        dimension: FlavorDimension,
    },
    Flavor {
        module: String,
        flavor: ProductFlavor,
    },
    Variant {
        module: String,
        variant: Variant,
    },
    Task {
        task: GradleTask,
    },
    Diagnostic {
        diagnostic: Diagnostic,
    },
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        suggested_action: Option<String>,
    },
    Complete {
        #[serde(flatten)]
        complete: BridgeComplete,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeComplete {
    pub duration_ms: u64,
    pub record_count: u64,
    pub model_hash: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BridgeProtocolError {
    #[error("bridge record is not valid JSON: {0}")]
    InvalidJson(String),
    #[error("unsupported bridge protocol version {found}; expected {expected}")]
    UnsupportedVersion { expected: u32, found: u32 },
    #[error("bridge stream contains more than one completion record")]
    DuplicateCompletion,
    #[error("bridge stream contains a record after completion")]
    RecordAfterCompletion,
    #[error("bridge completion expected {expected} records but received {actual}")]
    RecordCountMismatch { expected: u64, actual: u64 },
    #[error("bridge stream ended without a completion record")]
    MissingCompletion,
}

#[derive(Debug, Default)]
pub struct BridgeStreamValidator {
    record_count: u64,
    completion: Option<BridgeComplete>,
}

impl BridgeStreamValidator {
    pub fn accept_json_line(&mut self, line: &str) -> Result<BridgeEnvelope, BridgeProtocolError> {
        let record = serde_json::from_str::<BridgeEnvelope>(line)
            .map_err(|error| BridgeProtocolError::InvalidJson(error.to_string()))?;
        self.accept(&record)?;
        Ok(record)
    }

    pub fn accept(&mut self, record: &BridgeEnvelope) -> Result<(), BridgeProtocolError> {
        if record.protocol_version != BRIDGE_PROTOCOL_VERSION {
            return Err(BridgeProtocolError::UnsupportedVersion {
                expected: BRIDGE_PROTOCOL_VERSION,
                found: record.protocol_version,
            });
        }
        if self.completion.is_some() {
            return if matches!(record.payload, BridgePayload::Complete { .. }) {
                Err(BridgeProtocolError::DuplicateCompletion)
            } else {
                Err(BridgeProtocolError::RecordAfterCompletion)
            };
        }
        if let BridgePayload::Complete { complete } = &record.payload {
            if complete.record_count != self.record_count {
                return Err(BridgeProtocolError::RecordCountMismatch {
                    expected: complete.record_count,
                    actual: self.record_count,
                });
            }
            self.completion = Some(complete.clone());
        } else {
            self.record_count += 1;
        }
        Ok(())
    }

    pub fn finish(self) -> Result<BridgeComplete, BridgeProtocolError> {
        self.completion
            .ok_or(BridgeProtocolError::MissingCompletion)
    }
}
