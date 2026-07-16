use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use dexdeck_core::{ProcessResult, ProcessSupervisor, SensitiveValue, StreamingSecretRedactor};
use tokio::sync::{Mutex as AsyncMutex, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GradleArgumentLayers {
    pub shared: Vec<String>,
    pub user: Vec<String>,
    pub explicit: Vec<String>,
    pub profile: Vec<String>,
    pub cli: Vec<String>,
}

impl GradleArgumentLayers {
    pub fn resolve(&self) -> Result<Vec<String>, GradleRunError> {
        let values = self
            .shared
            .iter()
            .chain(&self.user)
            .chain(&self.explicit)
            .chain(&self.profile)
            .chain(&self.cli)
            .cloned()
            .collect::<Vec<_>>();
        validate_gradle_arguments(&values)?;
        Ok(values)
    }
}

pub struct GradleRunRequest {
    pub root: PathBuf,
    pub tasks: Vec<String>,
    pub arguments: GradleArgumentLayers,
    pub environment: BTreeMap<String, SensitiveValue>,
    pub cancel: CancellationToken,
    pub force_cancel: CancellationToken,
    pub output: Option<mpsc::Sender<GradleOutput>>,
    pub redactor: dexdeck_core::SecretRedactor,
}

impl std::fmt::Debug for GradleRunRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GradleRunRequest")
            .field("root", &self.root)
            .field("task_count", &self.tasks.len())
            .field("environment_keys", &self.environment.keys())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GradleOutput {
    pub stream: GradleOutputStream,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradleOutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct GradleTaskRunner {
    supervisor: ProcessSupervisor,
    roots: Arc<Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>>,
}

impl GradleTaskRunner {
    #[must_use]
    pub fn new(supervisor: ProcessSupervisor) -> Self {
        Self {
            supervisor,
            roots: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self, request: GradleRunRequest) -> Result<ProcessResult, GradleRunError> {
        if request.tasks.is_empty() || request.tasks.iter().any(String::is_empty) {
            return Err(GradleRunError::MissingTasks);
        }
        validate_gradle_arguments(&request.tasks)?;
        let root = request
            .root
            .canonicalize()
            .map_err(|source| GradleRunError::Root {
                path: request.root.clone(),
                source,
            })?;
        let wrapper = wrapper(&root);
        if !wrapper.is_file() {
            return Err(GradleRunError::MissingWrapper(wrapper));
        }
        let lock = {
            let mut roots = self
                .roots
                .lock()
                .map_err(|_| GradleRunError::SchedulerPoisoned)?;
            Arc::clone(
                roots
                    .entry(root.clone())
                    .or_insert_with(|| Arc::new(AsyncMutex::new(()))),
            )
        };
        let _guard = lock.lock().await;
        let mut arguments = request.tasks;
        arguments.extend(request.arguments.resolve()?);
        arguments.push("--console=plain".into());
        let mut spec = dexdeck_core::CommandSpec::new(wrapper, &root)?
            .args(arguments)
            .inherit_environment(true);
        for (name, value) in request.environment {
            spec = spec.env(name, value);
        }
        let result = self
            .supervisor
            .run(&spec, request.cancel, request.force_cancel)
            .await?;
        if let Some(sender) = request.output {
            emit_output(
                &sender,
                GradleOutputStream::Stdout,
                result.stdout.text_lossy(),
                request.redactor.clone(),
            )
            .await;
            emit_output(
                &sender,
                GradleOutputStream::Stderr,
                result.stderr.text_lossy(),
                request.redactor,
            )
            .await;
        }
        Ok(result)
    }
}

impl Default for GradleTaskRunner {
    fn default() -> Self {
        Self::new(ProcessSupervisor::default())
    }
}

async fn emit_output(
    sender: &mpsc::Sender<GradleOutput>,
    stream: GradleOutputStream,
    value: String,
    redactor: dexdeck_core::SecretRedactor,
) {
    let mut redactor = StreamingSecretRedactor::new(redactor);
    for chunk in value.as_bytes().chunks(8192) {
        let text = redactor.push(&String::from_utf8_lossy(chunk));
        if !text.is_empty() && sender.send(GradleOutput { stream, text }).await.is_err() {
            return;
        }
    }
    let text = redactor.finish();
    if !text.is_empty() {
        let _ = sender.send(GradleOutput { stream, text }).await;
    }
}

pub fn validate_gradle_arguments(arguments: &[String]) -> Result<(), GradleRunError> {
    for argument in arguments {
        let normalized = argument.to_ascii_lowercase();
        if argument == "-I"
            || matches!(
                normalized.as_str(),
                "--init-script"
                    | "--settings-file"
                    | "-c"
                    | "--project-dir"
                    | "-p"
                    | "--build-file"
                    | "-b"
            )
        {
            return Err(GradleRunError::ProtectedArgument(argument.clone()));
        }
        if normalized.starts_with("--init-script=")
            || normalized.starts_with("--settings-file=")
            || normalized.starts_with("--project-dir=")
            || normalized.starts_with("--build-file=")
            || normalized.starts_with("--console")
            || normalized == crate::MODEL_TASK.to_ascii_lowercase()
            || normalized.starts_with("-ddexdeck.output=")
            || normalized.starts_with("-ddexdeck.bridgejar=")
        {
            return Err(GradleRunError::ProtectedArgument(argument.clone()));
        }
    }
    Ok(())
}

fn wrapper(root: &Path) -> PathBuf {
    root.join(if cfg!(windows) {
        "gradlew.bat"
    } else {
        "gradlew"
    })
}

#[derive(Debug, thiserror::Error)]
pub enum GradleRunError {
    #[error(transparent)]
    Process(#[from] dexdeck_core::ProcessError),
    #[error("Gradle task list cannot be empty")]
    MissingTasks,
    #[error("Gradle argument {0:?} is reserved for DexDeck")]
    ProtectedArgument(String),
    #[error("project root {path:?} is invalid: {source}")]
    Root {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("project Gradle wrapper is missing at {0:?}")]
    MissingWrapper(PathBuf),
    #[error("Gradle root scheduler lock was poisoned")]
    SchedulerPoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_arguments_in_precedence_order_and_rejects_internal_controls() {
        let layers = GradleArgumentLayers {
            shared: vec!["--offline".into()],
            user: vec!["--no-offline".into()],
            explicit: vec!["--stacktrace".into()],
            profile: vec!["-Pprofile=yes".into()],
            cli: vec!["--rerun-tasks".into()],
        };
        assert_eq!(
            layers.resolve().unwrap_or_default(),
            [
                "--offline",
                "--no-offline",
                "--stacktrace",
                "-Pprofile=yes",
                "--rerun-tasks"
            ]
        );
        for values in [
            vec!["--init-script".into(), "evil.gradle".into()],
            vec!["--console=rich".into()],
            vec!["dexdeckModel".into()],
            vec!["-Ddexdeck.output=/tmp/stolen".into()],
        ] {
            assert!(matches!(
                validate_gradle_arguments(&values),
                Err(GradleRunError::ProtectedArgument(_))
            ));
        }
    }
}
