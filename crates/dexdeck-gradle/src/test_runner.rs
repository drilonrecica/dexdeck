use std::{collections::BTreeMap, path::PathBuf};

use dexdeck_core::{ProcessResult, SecretRedactor, SensitiveValue};
use dexdeck_protocol::TestSelection;
use tokio_util::sync::CancellationToken;

use crate::{GradleArgumentLayers, GradleRunError, GradleRunRequest, GradleTaskRunner};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AndroidTestKind {
    Local,
    Instrumentation,
    Custom { task: String },
}

#[derive(Debug)]
pub struct AndroidTestRequest {
    pub root: PathBuf,
    pub module: String,
    pub variant: String,
    pub kind: AndroidTestKind,
    pub selection: TestSelection,
    pub device_serial: Option<String>,
    pub gradle_arguments: GradleArgumentLayers,
    pub environment: BTreeMap<String, SensitiveValue>,
    pub cancel: CancellationToken,
    pub force_cancel: CancellationToken,
    pub redactor: SecretRedactor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedTestInvocation {
    pub task: String,
    pub arguments: Vec<String>,
    pub device_serial: Option<String>,
}

impl ResolvedTestInvocation {
    pub fn resolve(request: &AndroidTestRequest) -> Result<Self, TestRunError> {
        let module = normalize_module(&request.module)?;
        let variant = capitalize(&request.variant)?;
        let mut arguments = Vec::new();
        let (task, device_serial) = match &request.kind {
            AndroidTestKind::Local => {
                if request.selection.package.is_some() {
                    return Err(TestRunError::UnsupportedLocalPackageSelection);
                }
                if let Some(class) = &request.selection.class {
                    let test = request
                        .selection
                        .method
                        .as_ref()
                        .map_or_else(|| class.clone(), |method| format!("{class}.{method}"));
                    arguments.extend(["--tests".into(), test]);
                } else if request.selection.method.is_some() {
                    return Err(TestRunError::MethodWithoutClass);
                }
                (format!("{module}:test{variant}UnitTest"), None)
            }
            AndroidTestKind::Instrumentation => {
                let serial = request
                    .device_serial
                    .clone()
                    .ok_or(TestRunError::InstrumentationDeviceRequired)?;
                if let Some(package) = &request.selection.package {
                    arguments.push(format!(
                        "-Pandroid.testInstrumentationRunnerArguments.package={package}"
                    ));
                }
                if let Some(class) = &request.selection.class {
                    let value = request
                        .selection
                        .method
                        .as_ref()
                        .map_or_else(|| class.clone(), |method| format!("{class}#{method}"));
                    arguments.push(format!(
                        "-Pandroid.testInstrumentationRunnerArguments.class={value}"
                    ));
                } else if request.selection.method.is_some() {
                    return Err(TestRunError::MethodWithoutClass);
                }
                (
                    format!("{module}:connected{variant}AndroidTest"),
                    Some(serial),
                )
            }
            AndroidTestKind::Custom { task } => {
                if task.trim().is_empty() {
                    return Err(TestRunError::EmptyCustomTask);
                }
                if request.selection != TestSelection::default() {
                    return Err(TestRunError::CustomTaskSelectionAmbiguous);
                }
                (task.clone(), request.device_serial.clone())
            }
        };
        Ok(Self {
            task,
            arguments,
            device_serial,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct AndroidTestRunner {
    gradle: GradleTaskRunner,
}

impl AndroidTestRunner {
    #[must_use]
    pub fn new(gradle: GradleTaskRunner) -> Self {
        Self { gradle }
    }

    pub async fn run(
        &self,
        mut request: AndroidTestRequest,
    ) -> Result<ProcessResult, TestRunError> {
        let invocation = ResolvedTestInvocation::resolve(&request)?;
        request
            .gradle_arguments
            .cli
            .extend(invocation.arguments.clone());
        if let Some(serial) = invocation.device_serial {
            request
                .environment
                .insert("ANDROID_SERIAL".into(), SensitiveValue::new(serial));
        }
        self.gradle
            .run(GradleRunRequest {
                root: request.root,
                tasks: vec![invocation.task],
                arguments: request.gradle_arguments,
                environment: request.environment,
                cancel: request.cancel,
                force_cancel: request.force_cancel,
                output: None,
                redactor: request.redactor,
            })
            .await
            .map_err(TestRunError::Gradle)
    }
}

fn normalize_module(module: &str) -> Result<String, TestRunError> {
    let module = module.trim().trim_end_matches(':');
    if module.is_empty() || module.contains(char::is_whitespace) {
        return Err(TestRunError::InvalidModule);
    }
    Ok(if module.starts_with(':') {
        module.into()
    } else {
        format!(":{module}")
    })
}

fn capitalize(variant: &str) -> Result<String, TestRunError> {
    let mut characters = variant.chars();
    let first = characters.next().ok_or(TestRunError::InvalidVariant)?;
    if variant.contains(char::is_whitespace) {
        return Err(TestRunError::InvalidVariant);
    }
    Ok(first.to_uppercase().chain(characters).collect())
}

#[derive(Debug, thiserror::Error)]
pub enum TestRunError {
    #[error("test module is invalid")]
    InvalidModule,
    #[error("test variant is invalid")]
    InvalidVariant,
    #[error("test method selection requires a class")]
    MethodWithoutClass,
    #[error("local JVM package-only selection is unsupported; select a class or task")]
    UnsupportedLocalPackageSelection,
    #[error("instrumentation tests require an active device")]
    InstrumentationDeviceRequired,
    #[error("custom test task cannot be empty")]
    EmptyCustomTask,
    #[error("selection cannot be reconstructed for an arbitrary custom task")]
    CustomTaskSelectionAmbiguous,
    #[error(transparent)]
    Gradle(#[from] GradleRunError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(kind: AndroidTestKind) -> AndroidTestRequest {
        AndroidTestRequest {
            root: ".".into(),
            module: ":app".into(),
            variant: "debug".into(),
            kind,
            selection: TestSelection::default(),
            device_serial: None,
            gradle_arguments: GradleArgumentLayers::default(),
            environment: BTreeMap::new(),
            cancel: CancellationToken::new(),
            force_cancel: CancellationToken::new(),
            redactor: SecretRedactor::new(),
        }
    }

    #[test]
    fn resolves_local_class_and_method() -> Result<(), TestRunError> {
        let mut request = request(AndroidTestKind::Local);
        request.selection.class = Some("com.example.ExampleTest".into());
        request.selection.method = Some("passes".into());
        let invocation = ResolvedTestInvocation::resolve(&request)?;
        assert_eq!(invocation.task, ":app:testDebugUnitTest");
        assert_eq!(
            invocation.arguments,
            ["--tests", "com.example.ExampleTest.passes"]
        );
        Ok(())
    }

    #[test]
    fn instrumentation_requires_device_and_uses_runner_arguments() {
        let mut request = request(AndroidTestKind::Instrumentation);
        request.selection.package = Some("com.example".into());
        assert!(matches!(
            ResolvedTestInvocation::resolve(&request),
            Err(TestRunError::InstrumentationDeviceRequired)
        ));
        request.device_serial = Some("serial".into());
        let invocation =
            ResolvedTestInvocation::resolve(&request).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(invocation.task, ":app:connectedDebugAndroidTest");
        assert_eq!(invocation.device_serial.as_deref(), Some("serial"));
    }
}
