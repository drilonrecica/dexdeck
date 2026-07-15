use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use dexdeck_protocol::{CONFIG_SCHEMA_VERSION, LogPriority};
use serde::{Deserialize, Serialize};

use crate::ConfigError;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ConfigFile {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradle: Option<GradleConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui: Option<UiConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logcat: Option<LogcatConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<EditorConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, ProfileConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub commands: BTreeMap<String, CommandConfig>,
}

impl ConfigFile {
    pub fn validate(&self, path: PathBuf) -> Result<(), ConfigError> {
        if self.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedSchema {
                path,
                expected: CONFIG_SCHEMA_VERSION,
                found: self.schema_version,
            });
        }
        ConfigLayer::from(self.clone()).validate(path)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfigLayer {
    pub project: ProjectConfig,
    pub gradle: GradleConfig,
    pub ui: UiConfig,
    pub logcat: LogcatConfig,
    pub editor: EditorConfig,
    pub profiles: BTreeMap<String, ProfileConfig>,
    pub commands: BTreeMap<String, CommandConfig>,
}

impl From<ConfigFile> for ConfigLayer {
    fn from(config: ConfigFile) -> Self {
        Self {
            project: config.project.unwrap_or_default(),
            gradle: config.gradle.unwrap_or_default(),
            ui: config.ui.unwrap_or_default(),
            logcat: config.logcat.unwrap_or_default(),
            editor: config.editor.unwrap_or_default(),
            profiles: config.profiles,
            commands: config.commands,
        }
    }
}

impl ConfigLayer {
    pub fn validate(&self, path: PathBuf) -> Result<(), ConfigError> {
        validate_argv(&path, "gradle.arguments", self.gradle.arguments.as_deref())?;
        validate_argv(&path, "editor.command", self.editor.command.as_deref())?;

        if let Some(buffer_mib) = self.logcat.buffer_mib
            && !(8..=1024).contains(&buffer_mib)
        {
            return Err(ConfigError::Validation {
                path,
                field: "logcat.buffer_mib".into(),
                message: "must be between 8 and 1024 MiB".into(),
            });
        }

        for (name, profile) in &self.profiles {
            validate_environment(
                &path,
                &format!("profiles.{name}.environment"),
                &profile.environment,
            )?;
            validate_environment(
                &path,
                &format!("profiles.{name}.gradle_properties"),
                &profile.gradle_properties,
            )?;
        }
        for (name, command) in &self.commands {
            validate_argv(
                &path,
                &format!("commands.{name}.command"),
                Some(&command.command),
            )?;
            validate_environment(
                &path,
                &format!("commands.{name}.environment"),
                &command.environment,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_variant: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GradleConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeymapPreset {
    Default,
    Vim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnicodeMode {
    Auto,
    Unicode,
    Ascii,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UiConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keymap: Option<KeymapPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unicode: Option<UnicodeMode>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogScope {
    Application,
    Device,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LogcatConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_mib: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_priority: Option<LogPriority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_scope: Option<LogScope>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EditorConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaunchMode {
    Launcher,
    Activity,
    DeepLink,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProfileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_mode: Option<LaunchMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launcher_activity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deep_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_action: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intent_categories: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub intent_extras: BTreeMap<String, IntentExtra>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gradle_properties: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, EnvironmentValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_emulator_if_offline: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntentExtra {
    String(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvironmentValue {
    Literal(String),
    Integer(i64),
    Boolean(bool),
    FromEnvironment { from_env: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CommandConfig {
    pub command: Vec<String>,
    #[serde(default = "default_working_directory")]
    pub working_directory: PathBuf,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, EnvironmentValue>,
}

fn default_working_directory() -> PathBuf {
    PathBuf::from(".")
}

fn validate_argv(path: &Path, field: &str, argv: Option<&[String]>) -> Result<(), ConfigError> {
    let Some(argv) = argv else {
        return Ok(());
    };
    if argv.is_empty() || argv[0].is_empty() {
        return Err(ConfigError::Validation {
            path: path.to_path_buf(),
            field: field.into(),
            message: "must contain a non-empty executable and argv entries".into(),
        });
    }
    if argv.iter().any(|argument| argument.contains('\0')) {
        return Err(ConfigError::Validation {
            path: path.to_path_buf(),
            field: field.into(),
            message: "must not contain NUL bytes".into(),
        });
    }
    Ok(())
}

fn validate_environment(
    path: &Path,
    field: &str,
    environment: &BTreeMap<String, EnvironmentValue>,
) -> Result<(), ConfigError> {
    for (name, value) in environment {
        if !valid_environment_name(name) {
            return Err(ConfigError::Validation {
                path: path.to_path_buf(),
                field: format!("{field}.{name}"),
                message: "environment variable name is invalid".into(),
            });
        }
        if sensitive_name(name) && !matches!(value, EnvironmentValue::FromEnvironment { .. }) {
            return Err(ConfigError::Validation {
                path: path.to_path_buf(),
                field: format!("{field}.{name}"),
                message: "secret-like values must use { from_env = \"NAME\" }".into(),
            });
        }
        if let EnvironmentValue::FromEnvironment { from_env } = value
            && !valid_environment_name(from_env)
        {
            return Err(ConfigError::Validation {
                path: path.to_path_buf(),
                field: format!("{field}.{name}.from_env"),
                message: "referenced environment variable name is invalid".into(),
            });
        }
    }
    Ok(())
}

fn valid_environment_name(name: &str) -> bool {
    let mut characters = name.chars();
    matches!(characters.next(), Some('_' | 'A'..='Z' | 'a'..='z'))
        && characters.all(|character| matches!(character, '_' | 'A'..='Z' | 'a'..='z' | '0'..='9'))
}

fn sensitive_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    [
        "token",
        "password",
        "secret",
        "credential",
        "private_key",
        "api_key",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}
