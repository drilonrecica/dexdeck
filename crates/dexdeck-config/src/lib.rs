//! Configuration and local-state services.

mod atomic;
mod config_error;
mod envelope;
mod error;
mod identity;
mod model_cache;
mod parse;
mod paths;
mod resolve;
mod schema;

pub use atomic::{
    MAX_LOCAL_STATE_BYTES, RecoveredFile, load_json, load_json_recovering, write_json_atomic,
    write_text_atomic,
};
pub use config_error::{ConfigError, ConfigWarning};
pub use envelope::VersionedEnvelope;
pub use error::StorageError;
pub use identity::{PROJECT_NAMESPACE_VERSION, ProjectIdentity};
pub use model_cache::{
    ModelFingerprint, ModelInput, discover_model_inputs, fingerprint, load_model, save_model,
};
pub use parse::{
    ConfigDocument, ConfigScope, ParsedConfig, parse_config, write_config_document,
    write_config_migration,
};
pub use paths::{ProjectPaths, StoragePaths};
pub use resolve::{ConfigResolver, ResolvedConfig};
pub use schema::{
    CommandConfig, ConfigFile, ConfigLayer, EditorConfig, EnvironmentValue, GradleConfig,
    IntentExtra, KeymapPreset, LaunchMode, LogScope, LogcatConfig, ProfileConfig, ProjectConfig,
    UiConfig, UnicodeMode,
};
