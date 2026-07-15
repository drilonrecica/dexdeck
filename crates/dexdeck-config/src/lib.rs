//! Configuration and local-state services.

mod atomic;
mod envelope;
mod error;
mod identity;
mod paths;

pub use atomic::{
    MAX_LOCAL_STATE_BYTES, RecoveredFile, load_json, load_json_recovering, write_json_atomic,
};
pub use envelope::VersionedEnvelope;
pub use error::StorageError;
pub use identity::{PROJECT_NAMESPACE_VERSION, ProjectIdentity};
pub use paths::{ProjectPaths, StoragePaths};
