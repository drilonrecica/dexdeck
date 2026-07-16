use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use dexdeck_protocol::SourceLocation;
use tokio_util::sync::CancellationToken;

use crate::{CommandSpec, ProcessResult, ProcessSupervisor};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPreset {
    Zed,
    VisualStudioCode,
    Neovim,
    Vim,
    Helix,
    IntelliJ,
}

impl EditorPreset {
    #[must_use]
    pub fn command(self) -> Vec<String> {
        match self {
            Self::Zed => vec!["zed".into(), "{path}:{line}:{column}".into()],
            Self::VisualStudioCode => {
                vec![
                    "code".into(),
                    "--goto".into(),
                    "{path}:{line}:{column}".into(),
                ]
            }
            Self::Neovim => vec!["nvim".into(), "+{line}".into(), "{path}".into()],
            Self::Vim => vec!["vim".into(), "+{line}".into(), "{path}".into()],
            Self::Helix => vec!["hx".into(), "{path}:{line}:{column}".into()],
            Self::IntelliJ => vec![
                "idea".into(),
                "--line".into(),
                "{line}".into(),
                "{path}".into(),
            ],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorCommand {
    template: Vec<String>,
    executable: PathBuf,
}

impl EditorCommand {
    pub fn resolve(
        explicit: Option<Vec<String>>,
        environment: &BTreeMap<String, String>,
    ) -> Result<Self, EditorError> {
        let template = if let Some(explicit) = explicit {
            explicit
        } else if let Some(visual) = environment
            .get("VISUAL")
            .filter(|value| !value.trim().is_empty())
        {
            split_argv(visual)?
        } else if let Some(editor) = environment
            .get("EDITOR")
            .filter(|value| !value.trim().is_empty())
        {
            split_argv(editor)?
        } else {
            return Err(EditorError::NotConfigured);
        };
        validate_template(&template)?;
        let executable =
            find_executable(&template[0], environment.get("PATH").map(String::as_str))?;
        Ok(Self {
            template,
            executable,
        })
    }

    pub fn preset(
        preset: EditorPreset,
        environment: &BTreeMap<String, String>,
    ) -> Result<Self, EditorError> {
        Self::resolve(Some(preset.command()), environment)
    }

    pub fn argv(&self, location: &SourceLocation) -> Result<Vec<String>, EditorError> {
        let path = location.file.to_str().ok_or(EditorError::NonUtf8Path)?;
        let line = location.line.unwrap_or(1).max(1).to_string();
        let column = location.column.unwrap_or(1).max(1).to_string();
        Ok(self
            .template
            .iter()
            .skip(1)
            .map(|argument| {
                argument
                    .replace("{path}", path)
                    .replace("{line}", &line)
                    .replace("{column}", &column)
            })
            .collect())
    }
}

#[derive(Clone, Debug)]
pub struct EditorLauncher {
    supervisor: ProcessSupervisor,
}

impl EditorLauncher {
    #[must_use]
    pub fn new(supervisor: ProcessSupervisor) -> Self {
        Self { supervisor }
    }

    pub async fn open(
        &self,
        command: &EditorCommand,
        location: &SourceLocation,
        working_directory: &Path,
        cancel: CancellationToken,
    ) -> Result<ProcessResult, EditorError> {
        let spec = CommandSpec::new(&command.executable, working_directory)?
            .args(command.argv(location)?)
            .inherit_environment(true);
        self.supervisor
            .run(&spec, cancel, CancellationToken::new())
            .await
            .map_err(EditorError::Process)
    }
}

impl Default for EditorLauncher {
    fn default() -> Self {
        Self::new(ProcessSupervisor::default())
    }
}

fn validate_template(template: &[String]) -> Result<(), EditorError> {
    if template.is_empty() || template[0].trim().is_empty() {
        return Err(EditorError::EmptyCommand);
    }
    if !template.iter().any(|argument| argument.contains("{path}")) {
        return Err(EditorError::MissingPathPlaceholder);
    }
    for argument in template {
        let mut remaining = argument.as_str();
        while let Some(open) = remaining.find('{') {
            let after = &remaining[open..];
            let Some(close) = after.find('}') else {
                return Err(EditorError::InvalidPlaceholder(after.into()));
            };
            let placeholder = &after[..=close];
            if !matches!(placeholder, "{path}" | "{line}" | "{column}") {
                return Err(EditorError::InvalidPlaceholder(placeholder.into()));
            }
            remaining = &after[close + 1..];
        }
    }
    Ok(())
}

fn split_argv(value: &str) -> Result<Vec<String>, EditorError> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
        } else if character == '\\' && quote != Some('\'') {
            escaped = true;
        } else if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            } else {
                current.push(character);
            }
        } else if character.is_whitespace() && quote.is_none() {
            if !current.is_empty() {
                arguments.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if escaped || quote.is_some() {
        return Err(EditorError::MalformedEnvironmentCommand);
    }
    if !current.is_empty() {
        arguments.push(current);
    }
    Ok(arguments)
}

fn find_executable(command: &str, path: Option<&str>) -> Result<PathBuf, EditorError> {
    let candidate = PathBuf::from(command);
    if candidate.components().count() > 1 {
        return candidate
            .is_file()
            .then_some(candidate)
            .ok_or_else(|| EditorError::ExecutableMissing(command.into()));
    }
    let executable = std::env::consts::EXE_SUFFIX;
    path.unwrap_or_default()
        .split(if cfg!(windows) { ';' } else { ':' })
        .filter(|entry| !entry.is_empty())
        .flat_map(|entry| {
            let plain = Path::new(entry).join(command);
            let suffixed = Path::new(entry).join(format!("{command}{executable}"));
            [plain, suffixed]
        })
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| EditorError::ExecutableMissing(command.into()))
}

#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("no editor is configured; set editor.command, VISUAL, or EDITOR")]
    NotConfigured,
    #[error("editor command cannot be empty")]
    EmptyCommand,
    #[error("editor command must contain a {{path}} placeholder")]
    MissingPathPlaceholder,
    #[error("editor command contains invalid placeholder {0:?}")]
    InvalidPlaceholder(String),
    #[error("VISUAL or EDITOR contains unmatched quotes or an escape")]
    MalformedEnvironmentCommand,
    #[error("editor executable is unavailable: {0:?}")]
    ExecutableMissing(String),
    #[error("source path is not valid UTF-8")]
    NonUtf8Path,
    #[error(transparent)]
    Process(#[from] crate::ProcessError),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn visual_precedes_editor_and_is_parsed_without_a_shell()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let executable = directory.path().join("editor");
        fs::write(&executable, "")?;
        let environment = BTreeMap::from([
            (
                "PATH".into(),
                directory.path().to_string_lossy().into_owned(),
            ),
            (
                "VISUAL".into(),
                "editor --goto '{path}:{line}:{column}'".into(),
            ),
            ("EDITOR".into(), "missing {path}".into()),
        ]);
        let command = EditorCommand::resolve(None, &environment)?;
        let argv = command.argv(&SourceLocation {
            file: "src/App.kt".into(),
            line: Some(4),
            column: Some(2),
        })?;
        assert_eq!(argv, ["--goto", "src/App.kt:4:2"]);
        Ok(())
    }

    #[test]
    fn rejects_unknown_placeholders_and_missing_executables() {
        let environment = BTreeMap::new();
        assert!(matches!(
            EditorCommand::resolve(
                Some(vec!["editor".into(), "{unknown}".into()]),
                &environment
            ),
            Err(EditorError::MissingPathPlaceholder | EditorError::InvalidPlaceholder(_))
        ));
    }
}
