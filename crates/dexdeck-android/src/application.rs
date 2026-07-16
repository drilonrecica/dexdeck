use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use dexdeck_config::IntentExtra;
use dexdeck_core::LaunchRequest;
use dexdeck_protocol::{AndroidModule, ArtifactKind, Variant};
use serde::Deserialize;

use crate::{AdbClient, AdbError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstallOptions {
    pub downgrade: bool,
    pub grant_all: bool,
}

#[derive(Clone, Debug)]
pub struct ApplicationService {
    adb: Arc<AdbClient>,
}

impl ApplicationService {
    #[must_use]
    pub fn new(adb: Arc<AdbClient>) -> Self {
        Self { adb }
    }

    pub fn discover_apks(
        &self,
        module: &AndroidModule,
        variant: &Variant,
    ) -> Result<Vec<PathBuf>, ApplicationError> {
        let module_root = module.project_directory.canonicalize().map_err(|source| {
            ApplicationError::ModuleDirectory {
                path: module.project_directory.clone(),
                source,
            }
        })?;
        let build_root = module_root.join("build").canonicalize().map_err(|source| {
            ApplicationError::BuildDirectory {
                path: module_root.join("build"),
                source,
            }
        })?;
        let mut apks = Vec::new();
        let mut saw_bundle = false;
        for artifact in &variant.artifacts {
            match artifact.kind {
                ArtifactKind::Apk => {
                    let path = resolve_artifact(&module_root, &build_root, &artifact.path)?;
                    apks.push((artifact.filters.is_empty(), path));
                }
                ArtifactKind::Bundle => saw_bundle = true,
                _ => {}
            }
        }
        if apks.is_empty() {
            for metadata in metadata_files(&build_root.join("outputs/apk"))? {
                let parent = metadata
                    .parent()
                    .ok_or_else(|| ApplicationError::InvalidArtifact(metadata.clone()))?;
                let value: OutputMetadata =
                    serde_json::from_slice(&fs::read(&metadata).map_err(|source| {
                        ApplicationError::Metadata {
                            path: metadata.clone(),
                            source,
                        }
                    })?)
                    .map_err(|source| {
                        ApplicationError::InvalidMetadata {
                            path: metadata.clone(),
                            source,
                        }
                    })?;
                for element in value.elements {
                    let path = resolve_artifact(
                        &module_root,
                        &build_root,
                        &parent.join(element.output_file),
                    )?;
                    apks.push((element.filters.is_empty(), path));
                }
            }
        }
        if apks.is_empty() {
            return if saw_bundle {
                Err(ApplicationError::BundleNotInstallable)
            } else {
                Err(ApplicationError::ArtifactMissing(variant.name.clone()))
            };
        }
        apks.sort_by(|(left_base, left), (right_base, right)| {
            right_base.cmp(left_base).then_with(|| left.cmp(right))
        });
        apks.dedup_by(|left, right| left.1 == right.1);
        Ok(apks.into_iter().map(|(_, path)| path).collect())
    }

    pub async fn install(
        &self,
        serial: &str,
        apks: &[PathBuf],
        options: InstallOptions,
    ) -> Result<(), ApplicationError> {
        if apks.is_empty() {
            return Err(ApplicationError::ArtifactMissing("install".into()));
        }
        let mut arguments = vec!["-s".into(), serial.into()];
        arguments.push(
            if apks.len() == 1 {
                "install"
            } else {
                "install-multiple"
            }
            .into(),
        );
        arguments.push("-r".into());
        if options.downgrade {
            arguments.push("-d".into());
        }
        if options.grant_all {
            arguments.push("-g".into());
        }
        arguments.extend(apks.iter().map(|path| path.display().to_string()));
        self.adb.command(&arguments).await?;
        Ok(())
    }

    pub async fn launch(
        &self,
        serial: &str,
        variant: &Variant,
        request: &LaunchRequest,
    ) -> Result<(), ApplicationError> {
        let package = variant
            .application_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or(ApplicationError::MissingPackage)?;
        let mut shell = vec!["am".into(), "start".into(), "-W".into()];
        match request {
            LaunchRequest::Activity { activity } => {
                shell.extend(["-n".into(), component(package, activity)]);
            }
            LaunchRequest::Launcher => {
                let component = if let Some(launcher) = &variant.launcher {
                    component(
                        launcher.package.as_deref().unwrap_or(package),
                        &launcher.activity,
                    )
                } else {
                    self.resolve_activity(serial, package).await?
                };
                shell.extend(["-n".into(), component]);
            }
            LaunchRequest::DeepLink {
                uri,
                action,
                categories,
                extras,
            } => {
                shell.extend([
                    "-a".into(),
                    action
                        .clone()
                        .unwrap_or_else(|| "android.intent.action.VIEW".into()),
                    "-d".into(),
                    uri.clone(),
                ]);
                for category in categories {
                    shell.extend(["-c".into(), category.clone()]);
                }
                for (name, value) in extras {
                    match value {
                        IntentExtra::String(value) => {
                            shell.extend(["--es".into(), name.clone(), value.clone()])
                        }
                        IntentExtra::Integer(value) => {
                            shell.extend(["--el".into(), name.clone(), value.to_string()])
                        }
                        IntentExtra::Boolean(value) => {
                            shell.extend(["--ez".into(), name.clone(), value.to_string()])
                        }
                    }
                }
                shell.push(package.into());
            }
        }
        let mut arguments = vec!["-s".into(), serial.into(), "shell".into()];
        arguments.extend(shell);
        self.adb.command(&arguments).await?;
        Ok(())
    }

    async fn resolve_activity(
        &self,
        serial: &str,
        package: &str,
    ) -> Result<String, ApplicationError> {
        let output = self
            .adb
            .command(&[
                "-s".into(),
                serial.into(),
                "shell".into(),
                "cmd".into(),
                "package".into(),
                "resolve-activity".into(),
                "--brief".into(),
                package.into(),
            ])
            .await?;
        output
            .lines()
            .map(str::trim)
            .find(|line| line.contains('/') && !line.contains("No activity"))
            .map(str::to_owned)
            .ok_or_else(|| ApplicationError::MissingActivity(package.into()))
    }

    pub async fn force_stop(&self, serial: &str, package: &str) -> Result<(), ApplicationError> {
        self.package_command(serial, &["am", "force-stop", package])
            .await
    }

    pub async fn uninstall(&self, serial: &str, package: &str) -> Result<(), ApplicationError> {
        self.adb
            .command(&[
                "-s".into(),
                serial.into(),
                "uninstall".into(),
                package.into(),
            ])
            .await?;
        Ok(())
    }

    pub async fn clear_data(&self, serial: &str, package: &str) -> Result<(), ApplicationError> {
        self.package_command(serial, &["pm", "clear", package])
            .await
    }

    async fn package_command(
        &self,
        serial: &str,
        command: &[&str],
    ) -> Result<(), ApplicationError> {
        let mut arguments = vec!["-s".into(), serial.into(), "shell".into()];
        arguments.extend(command.iter().map(|value| (*value).into()));
        self.adb.command(&arguments).await?;
        Ok(())
    }
}

fn component(package: &str, activity: &str) -> String {
    if activity.contains('/') {
        activity.into()
    } else {
        format!("{package}/{activity}")
    }
}

fn resolve_artifact(module: &Path, build: &Path, path: &Path) -> Result<PathBuf, ApplicationError> {
    let path = if path.is_absolute() {
        path.into()
    } else {
        module.join(path)
    };
    let canonical = path
        .canonicalize()
        .map_err(|source| ApplicationError::ArtifactIo {
            path: path.clone(),
            source,
        })?;
    if !canonical.starts_with(build)
        || canonical.extension().and_then(|value| value.to_str()) != Some("apk")
    {
        return Err(ApplicationError::InvalidArtifact(canonical));
    }
    Ok(canonical)
}

fn metadata_files(root: &Path) -> Result<Vec<PathBuf>, ApplicationError> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut pending = vec![root.to_path_buf()];
    let mut values = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).map_err(|source| ApplicationError::Metadata {
            path: directory.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| ApplicationError::Metadata {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.file_name().and_then(|value| value.to_str())
                == Some("output-metadata.json")
            {
                values.push(path);
            }
        }
    }
    values.sort();
    Ok(values)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputMetadata {
    elements: Vec<OutputElement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputElement {
    output_file: PathBuf,
    #[serde(default)]
    filters: Vec<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Adb(#[from] AdbError),
    #[error("module directory {path:?} is invalid: {source}")]
    ModuleDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("module build directory {path:?} is unavailable: {source}")]
    BuildDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("artifact for variant {0:?} is missing")]
    ArtifactMissing(String),
    #[error("Android App Bundles cannot be installed directly")]
    BundleNotInstallable,
    #[error("artifact path {0:?} is outside the module build tree or is not an APK")]
    InvalidArtifact(PathBuf),
    #[error("artifact {path:?} is unavailable: {source}")]
    ArtifactIo {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read APK metadata {path:?}: {source}")]
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid APK metadata {path:?}: {source}")]
    InvalidMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("application ID is missing")]
    MissingPackage,
    #[error("no launchable activity is installed for {0:?}")]
    MissingActivity(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use dexdeck_core::ProcessSupervisor;
    use dexdeck_protocol::{AndroidModule, Artifact, ModuleKind, VariantTasks};

    #[test]
    fn orders_base_before_splits_and_rejects_escape() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let module_root = temp.path().join("app");
        let output = module_root.join("build/outputs/apk/debug");
        fs::create_dir_all(&output)?;
        fs::write(output.join("base.apk"), [])?;
        fs::write(output.join("split.apk"), [])?;
        let module = AndroidModule {
            path: ":app".into(),
            build_id: "root".into(),
            project_directory: module_root,
            kind: ModuleKind::Application,
            namespace: None,
            compile_sdk: None,
            target_sdk: None,
            minimum_sdk: None,
            flavor_dimensions: vec![],
            product_flavors: vec![],
            build_types: vec![],
            variants: vec![],
        };
        let variant = Variant {
            name: "debug".into(),
            enabled: true,
            build_type: "debug".into(),
            flavors: vec![],
            application_id: Some("dev.example".into()),
            namespace: None,
            debuggable: Some(true),
            launcher: None,
            tasks: VariantTasks::default(),
            artifacts: vec![
                Artifact {
                    kind: ArtifactKind::Apk,
                    path: PathBuf::from("build/outputs/apk/debug/split.apk"),
                    filters: vec!["x86".into()],
                },
                Artifact {
                    kind: ArtifactKind::Apk,
                    path: PathBuf::from("build/outputs/apk/debug/base.apk"),
                    filters: vec![],
                },
            ],
            test_components: vec![],
        };
        let adb = Arc::new(AdbClient::new(
            "adb".into(),
            temp.path().into(),
            ProcessSupervisor::default(),
        ));
        let apks = ApplicationService::new(adb).discover_apks(&module, &variant)?;
        assert!(apks[0].ends_with("base.apk"));
        Ok(())
    }
}
