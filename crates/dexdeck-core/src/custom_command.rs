use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use dexdeck_config::{
    CommandConfig, EnvironmentValue, ProjectIdentity, RecoveredFile, VersionedEnvelope,
    load_json_recovering, write_json_atomic,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::{CommandSpec, ProcessResult, ProcessSupervisor, SecretRedactor, SensitiveValue};

pub const TRUST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustDecision {
    Once,
    Project,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPreview {
    pub argv: Vec<String>,
    pub working_directory: PathBuf,
    pub already_trusted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustFingerprint {
    pub project_identity: String,
    pub canonical_root_hash: String,
    pub git_remote_hash: String,
}

#[derive(Clone, Debug)]
pub struct CustomCommandService {
    root: PathBuf,
    trust_path: PathBuf,
    supervisor: ProcessSupervisor,
}

impl CustomCommandService {
    pub fn new(
        root: impl AsRef<Path>,
        trust_path: PathBuf,
        supervisor: ProcessSupervisor,
    ) -> Result<Self, CustomCommandError> {
        let root = root
            .as_ref()
            .canonicalize()
            .map_err(|source| CustomCommandError::Root {
                path: root.as_ref().into(),
                source,
            })?;
        Ok(Self {
            root,
            trust_path,
            supervisor,
        })
    }

    pub fn preview(
        &self,
        command: &CommandConfig,
        redactor: &SecretRedactor,
    ) -> Result<CommandPreview, CustomCommandError> {
        let working_directory = self.working_directory(command)?;
        Ok(CommandPreview {
            argv: redactor.redact_argv(&command.command),
            working_directory,
            already_trusted: self.is_trusted()?,
        })
    }

    pub async fn execute(
        &self,
        command: &CommandConfig,
        decision: TrustDecision,
        interactive: bool,
        redactor: &mut SecretRedactor,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
    ) -> Result<ProcessResult, CustomCommandError> {
        let trusted = self.is_trusted()?;
        if !trusted {
            if !interactive {
                return Err(CustomCommandError::TrustRequired);
            }
            match decision {
                TrustDecision::Cancel => return Err(CustomCommandError::Cancelled),
                TrustDecision::Once => {}
                TrustDecision::Project => self.store_trust()?,
            }
        }
        let working_directory = self.working_directory(command)?;
        let (program, arguments) = command
            .command
            .split_first()
            .ok_or(CustomCommandError::Empty)?;
        let mut spec = CommandSpec::new(program, working_directory)?
            .args(arguments)
            .inherit_environment(true);
        for (name, value) in resolve_environment(&command.environment, redactor)? {
            spec = spec.env(name, value);
        }
        Ok(self.supervisor.run(&spec, cancel, force_cancel).await?)
    }

    fn working_directory(&self, command: &CommandConfig) -> Result<PathBuf, CustomCommandError> {
        let candidate = if command.working_directory.is_absolute() {
            command.working_directory.clone()
        } else {
            self.root.join(&command.working_directory)
        };
        let canonical =
            candidate
                .canonicalize()
                .map_err(|source| CustomCommandError::WorkingDirectory {
                    path: candidate,
                    source,
                })?;
        if !canonical.starts_with(&self.root) {
            return Err(CustomCommandError::WorkingDirectoryEscape(canonical));
        }
        Ok(canonical)
    }

    fn is_trusted(&self) -> Result<bool, CustomCommandError> {
        let expected = self.fingerprint()?;
        Ok(matches!(
            load_json_recovering::<TrustFingerprint>(&self.trust_path, TRUST_SCHEMA_VERSION)?,
            RecoveredFile::Loaded(stored) if stored == expected
        ))
    }

    fn store_trust(&self) -> Result<(), CustomCommandError> {
        write_json_atomic(
            &self.trust_path,
            &VersionedEnvelope::new(TRUST_SCHEMA_VERSION, self.fingerprint()?),
        )?;
        Ok(())
    }

    fn fingerprint(&self) -> Result<TrustFingerprint, CustomCommandError> {
        let identity = ProjectIdentity::from_path(&self.root)?;
        let remote = git_remote(&self.root)?;
        Ok(TrustFingerprint {
            project_identity: identity.hash().into(),
            canonical_root_hash: hash_path(&self.root),
            git_remote_hash: hash_bytes(remote.as_bytes()),
        })
    }
}

fn resolve_environment(
    values: &BTreeMap<String, EnvironmentValue>,
    redactor: &mut SecretRedactor,
) -> Result<BTreeMap<String, SensitiveValue>, CustomCommandError> {
    values
        .iter()
        .map(|(name, value)| {
            let value = match value {
                EnvironmentValue::Literal(value) => SensitiveValue::new(value),
                EnvironmentValue::Integer(value) => SensitiveValue::new(value.to_string()),
                EnvironmentValue::Boolean(value) => SensitiveValue::new(value.to_string()),
                EnvironmentValue::FromEnvironment { from_env } => {
                    SensitiveValue::from_environment(from_env)?
                }
            };
            redactor.register(&value);
            Ok((name.clone(), value))
        })
        .collect()
}

fn git_remote(root: &Path) -> Result<String, CustomCommandError> {
    let path = root.join(".git/config");
    let value = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(source) => return Err(CustomCommandError::GitConfig { path, source }),
    };
    let mut in_origin = false;
    for line in value.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_origin = line == "[remote \"origin\"]";
        } else if in_origin
            && let Some(("url", value)) = line
                .split_once('=')
                .map(|(key, value)| (key.trim(), value.trim()))
        {
            return Ok(value.into());
        }
    }
    Ok(String::new())
}

#[cfg(unix)]
fn hash_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    hash_bytes(path.as_os_str().as_bytes())
}

#[cfg(windows)]
fn hash_path(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    let bytes = path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    hash_bytes(&bytes)
}

fn hash_bytes(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

#[derive(Debug, thiserror::Error)]
pub enum CustomCommandError {
    #[error("project root {path:?} is invalid: {source}")]
    Root {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("custom command working directory {path:?} is invalid: {source}")]
    WorkingDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("custom command working directory escapes the project root: {0:?}")]
    WorkingDirectoryEscape(PathBuf),
    #[error("custom command argv is empty")]
    Empty,
    #[error(
        "trust.required: establish project trust interactively before machine-readable execution"
    )]
    TrustRequired,
    #[error("custom command was cancelled before trust was granted")]
    Cancelled,
    #[error("failed to read Git configuration {path:?}: {source}")]
    GitConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(transparent)]
    Secret(#[from] crate::SecretError),
    #[error(transparent)]
    Process(#[from] crate::ProcessError),
    #[error(transparent)]
    Storage(#[from] dexdeck_config::StorageError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_invalidates_when_remote_changes_and_cwd_escape_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        fs::create_dir(temp.path().join(".git"))?;
        fs::write(
            temp.path().join(".git/config"),
            "[remote \"origin\"]\n url = first\n",
        )?;
        let service = CustomCommandService::new(
            temp.path(),
            temp.path().join("trust.json"),
            ProcessSupervisor::default(),
        )?;
        service.store_trust()?;
        assert!(service.is_trusted()?);
        fs::write(
            temp.path().join(".git/config"),
            "[remote \"origin\"]\n url = second\n",
        )?;
        assert!(!service.is_trusted()?);
        let outside = tempfile::tempdir()?;
        let command = CommandConfig {
            command: vec!["tool".into()],
            working_directory: outside.path().into(),
            environment: BTreeMap::new(),
        };
        assert!(matches!(
            service.preview(&command, &SecretRedactor::new()),
            Err(CustomCommandError::WorkingDirectoryEscape(_))
        ));
        Ok(())
    }
}
