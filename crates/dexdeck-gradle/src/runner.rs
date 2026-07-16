use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use dexdeck_core::{
    CommandSpec, ProcessError, ProcessSupervisor, SecretRedactor, TerminationReason,
};
use dexdeck_protocol::{BridgeEnvelope, BridgePayload, ProjectModel};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::{
    BridgeInvocation, EmbeddedBridgeError, ModelAssembler, ModelAssemblyError, extract_bridge,
    select_gradle,
};

static OUTPUT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeRunOutput {
    pub model: ProjectModel,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Error)]
pub enum BridgeFailure {
    #[error(transparent)]
    Embedded(#[from] EmbeddedBridgeError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error("bridge was cancelled")]
    Cancelled,
    #[error("Gradle bridge exited with code {code:?}: {stderr}")]
    Exit { code: Option<i32>, stderr: String },
    #[error("bridge reported {code}: {message}")]
    Reported { code: String, message: String },
    #[error("bridge did not create its JSONL output")]
    MissingOutput,
    #[error("failed to read bridge output: {0}")]
    OutputIo(#[source] io::Error),
    #[error(transparent)]
    Assembly(#[from] ModelAssemblyError),
}

#[derive(Clone, Debug)]
pub struct BridgeRunner {
    cache_directory: PathBuf,
    process: ProcessSupervisor,
    system_gradle_approved: bool,
}

impl BridgeRunner {
    #[must_use]
    pub fn new(cache_directory: PathBuf, process: ProcessSupervisor) -> Self {
        Self {
            cache_directory,
            process,
            system_gradle_approved: false,
        }
    }

    #[must_use]
    pub const fn approve_system_gradle(mut self, approved: bool) -> Self {
        self.system_gradle_approved = approved;
        self
    }

    pub async fn run(
        &self,
        root: &Path,
        wrapper: Option<&Path>,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
        redactor: &SecretRedactor,
    ) -> Result<BridgeRunOutput, BridgeFailure> {
        let executable = select_gradle(wrapper, self.system_gradle_approved)?;
        let extracted = extract_bridge(&self.cache_directory)?;
        let sequence = OUTPUT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let output = extracted
            .directory
            .join(format!("model-{}-{sequence}.jsonl", std::process::id()));
        let _cleanup = PartialOutput::new(output.clone());
        let invocation = BridgeInvocation::new(&executable, &extracted.init_script, &output);
        let spec = CommandSpec::new(&invocation.executable, root)?
            .args(invocation.arguments)
            .inherit_environment(true);
        let result = self.process.run(&spec, cancel, force_cancel).await?;
        let stdout = redactor.redact_text(&result.stdout.text_lossy());
        let stderr = redactor.redact_text(&result.stderr.text_lossy());
        if result.termination != TerminationReason::Exited {
            return Err(BridgeFailure::Cancelled);
        }
        if result.exit_code != Some(0) {
            if let Some(failure) = read_reported_failure(&output, redactor) {
                return Err(failure);
            }
            return Err(BridgeFailure::Exit {
                code: result.exit_code,
                stderr,
            });
        }
        let text = match fs::read_to_string(&output) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(BridgeFailure::MissingOutput);
            }
            Err(error) => return Err(BridgeFailure::OutputIo(error)),
        };
        let mut assembler = ModelAssembler::default();
        for line in text.lines() {
            let record = serde_json::from_str::<BridgeEnvelope>(line).map_err(|error| {
                ModelAssemblyError::Protocol(dexdeck_protocol::BridgeProtocolError::InvalidJson(
                    error.to_string(),
                ))
            })?;
            if let BridgePayload::Error { code, message, .. } = &record.payload {
                return Err(BridgeFailure::Reported {
                    code: redactor.redact_text(code),
                    message: redactor.redact_text(message),
                });
            }
            assembler.accept(record)?;
        }
        let model = assembler.finish()?;
        Ok(BridgeRunOutput {
            model,
            stdout,
            stderr,
        })
    }
}

fn read_reported_failure(path: &Path, redactor: &SecretRedactor) -> Option<BridgeFailure> {
    let text = fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        let record = serde_json::from_str::<BridgeEnvelope>(line).ok()?;
        if let BridgePayload::Error { code, message, .. } = record.payload {
            Some(BridgeFailure::Reported {
                code: redactor.redact_text(&code),
                message: redactor.redact_text(&message),
            })
        } else {
            None
        }
    })
}

#[derive(Debug)]
struct PartialOutput(PathBuf);

impl PartialOutput {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }
}

impl Drop for PartialOutput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{os::unix::fs::PermissionsExt, time::Duration};

    fn wrapper(directory: &Path, body: &str) -> io::Result<PathBuf> {
        let path = directory.join("gradlew");
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
        Ok(path)
    }

    fn no_partial_outputs(cache: &Path) -> io::Result<bool> {
        for directory in fs::read_dir(cache)? {
            let directory = directory?.path();
            if directory.is_dir()
                && fs::read_dir(directory)?.any(|entry| {
                    entry.ok().is_some_and(|entry| {
                        entry.file_name().to_string_lossy().starts_with("model-")
                    })
                })
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[tokio::test]
    async fn nonzero_exit_deletes_partial_protocol_output() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let cache = directory.path().join("cache");
        let wrapper = wrapper(
            directory.path(),
            "for arg in \"$@\"; do case \"$arg\" in -Ddexdeck.output=*) out=${arg#*=};; esac; done\nprintf 'partial\\n' > \"$out\"\nexit 1",
        )?;
        let runner = BridgeRunner::new(
            cache.clone(),
            ProcessSupervisor::new(1024, Duration::from_millis(100))?,
        );
        assert!(matches!(
            runner
                .run(
                    directory.path(),
                    Some(&wrapper),
                    CancellationToken::new(),
                    CancellationToken::new(),
                    &SecretRedactor::new()
                )
                .await,
            Err(BridgeFailure::Exit { .. })
        ));
        assert!(no_partial_outputs(&cache)?);
        Ok(())
    }
}
