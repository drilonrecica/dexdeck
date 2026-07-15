use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("operating-system configuration and cache directories are unavailable")]
    PlatformDirectoriesUnavailable,
    #[error("unsafe symbolic link at {path}")]
    UnsafePath { path: PathBuf },
    #[error("local state at {path} is {size} bytes; maximum is {maximum} bytes")]
    FileTooLarge {
        path: PathBuf,
        size: u64,
        maximum: u64,
    },
    #[error("local state at {path} uses schema version {found}; expected {expected}")]
    UnsupportedSchema {
        path: PathBuf,
        expected: u32,
        found: u32,
    },
    #[error("local state at {path} is corrupt: {message}")]
    CorruptData { path: PathBuf, message: String },
    #[error("failed to serialize local state for {path}: {source}")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("I/O operation failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl StorageError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
