//! Configuration and local-state services.

mod atomic;
mod config_error;
mod envelope;
mod error;
mod identity;
mod job_history;
mod loader;
mod model_cache;
mod model_watch;
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
pub use job_history::{PERSISTED_JOB_HISTORY_LIMIT, load_job_history, save_job_history};
pub use loader::{ConfigLoader, ConfigSources, LoadedConfig};
pub use model_cache::{
    ModelCacheBundle, ModelFingerprint, ModelInput, discover_model_inputs, fingerprint,
    fingerprint_for_model, load_fingerprint, load_model, load_model_bundle, save_fingerprint,
    save_model, save_model_bundle,
};
pub use model_watch::{
    DEFAULT_MODEL_DEBOUNCE, ModelInputWatcher, ModelWatchError, ModelWatchState, SessionSelection,
    WatchDecision, is_model_input,
};
pub use parse::{
    ConfigDocument, ConfigScope, ParsedConfig, parse_config, write_config_document,
    write_config_migration,
};
pub use paths::{ProjectPaths, StoragePaths};
pub use resolve::{ConfigResolver, ResolvedConfig};
pub use schema::{
    AndroidConfig, CommandConfig, ConfigFile, ConfigLayer, EditorConfig, EnvironmentValue,
    GradleConfig, IntentExtra, KeymapPreset, LaunchMode, LogScope, LogcatConfig, ProfileConfig,
    ProjectConfig, UiConfig, UnicodeMode,
};
