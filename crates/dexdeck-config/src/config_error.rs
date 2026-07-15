use std::path::PathBuf;

use thiserror::Error;

use crate::StorageError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigWarning {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{path}:{line}:{column}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("{path}: unsupported configuration schema {found}; expected {expected}")]
    UnsupportedSchema {
        path: PathBuf,
        expected: u32,
        found: u32,
    },
    #[error("{path}: invalid configuration field {field}: {message}")]
    Validation {
        path: PathBuf,
        field: String,
        message: String,
    },
    #[error("shared configuration migration requires explicit confirmation: {path}")]
    SharedMigrationConfirmationRequired { path: PathBuf },
    #[error(transparent)]
    Storage(#[from] StorageError),
}
