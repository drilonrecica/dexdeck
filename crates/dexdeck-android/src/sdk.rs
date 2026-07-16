use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use dexdeck_protocol::{AndroidTools, DoctorCheck, DoctorSnapshot, DoctorStatus, SdkSource};

#[derive(Clone, Debug, Default)]
pub struct SdkResolution {
    pub cli: Option<PathBuf>,
    pub configuration: Option<PathBuf>,
    pub model: Option<PathBuf>,
    pub project_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub struct SdkResolver {
    environment: Option<BTreeMap<OsString, OsString>>,
    home: Option<PathBuf>,
}

impl SdkResolver {
    #[must_use]
    pub fn with_environment(
        environment: BTreeMap<OsString, OsString>,
        home: Option<PathBuf>,
    ) -> Self {
        Self {
            environment: Some(environment),
            home,
        }
    }

    pub fn resolve(&self, request: &SdkResolution) -> Result<AndroidTools, SdkError> {
        let candidates = self.candidates(request)?;
        let (root, source) = candidates.into_iter().next().ok_or(SdkError::MissingSdk)?;
        let root = root
            .canonicalize()
            .map_err(|source_error| SdkError::InvalidSdk {
                path: root,
                source: source_error,
            })?;
        if !root.is_dir() {
            return Err(SdkError::InvalidLayout(root));
        }
        let executable = |path: PathBuf| platform_executable(path);
        let java = self.resolve_java().ok_or(SdkError::MissingJava)?;
        Ok(AndroidTools {
            adb: executable(root.join("platform-tools/adb")),
            emulator: executable(root.join("emulator/emulator")),
            sdkmanager: executable(root.join("cmdline-tools/latest/bin/sdkmanager")),
            avdmanager: executable(root.join("cmdline-tools/latest/bin/avdmanager")),
            sdk_root: root,
            source,
            java,
        })
    }

    pub fn conflicts(&self) -> Vec<PathBuf> {
        let root = self.variable("ANDROID_SDK_ROOT").map(PathBuf::from);
        let home = self.variable("ANDROID_HOME").map(PathBuf::from);
        match (root, home) {
            (Some(root), Some(home)) if normalized(&root) != normalized(&home) => vec![root, home],
            _ => Vec::new(),
        }
    }

    fn candidates(&self, request: &SdkResolution) -> Result<Vec<(PathBuf, SdkSource)>, SdkError> {
        let mut values = Vec::new();
        push(&mut values, request.cli.clone(), SdkSource::Cli);
        push(
            &mut values,
            request.configuration.clone(),
            SdkSource::Configuration,
        );
        push(&mut values, request.model.clone(), SdkSource::Model);
        if let Some(root) = &request.project_root
            && let Some(path) = read_local_properties(&root.join("local.properties"))?
        {
            push(&mut values, Some(path), SdkSource::LocalProperties);
        }
        push(
            &mut values,
            self.variable("ANDROID_SDK_ROOT").map(PathBuf::from),
            SdkSource::AndroidSdkRoot,
        );
        push(
            &mut values,
            self.variable("ANDROID_HOME").map(PathBuf::from),
            SdkSource::AndroidHome,
        );
        for path in self.platform_defaults() {
            if path.is_dir() {
                push(&mut values, Some(path), SdkSource::PlatformDefault);
            }
        }
        Ok(values)
    }

    fn resolve_java(&self) -> Option<PathBuf> {
        if let Some(home) = self.variable("JAVA_HOME") {
            let path = platform_executable(PathBuf::from(home).join("bin/java"));
            if path.is_file() {
                return Some(path);
            }
        }
        let path = self.variable("PATH")?;
        env::split_paths(&path)
            .map(|entry| platform_executable(entry.join("java")))
            .find(|candidate| candidate.is_file())
    }

    fn variable(&self, name: &str) -> Option<OsString> {
        self.environment
            .as_ref()
            .and_then(|values| values.get(OsStr::new(name)).cloned())
            .or_else(|| {
                self.environment
                    .is_none()
                    .then(|| env::var_os(name))
                    .flatten()
            })
    }

    fn platform_defaults(&self) -> Vec<PathBuf> {
        let home = self
            .home
            .clone()
            .or_else(|| self.variable("HOME").map(PathBuf::from));
        let mut values = Vec::new();
        if cfg!(target_os = "macos") {
            if let Some(home) = home {
                values.push(home.join("Library/Android/sdk"));
            }
        } else if cfg!(windows) {
            if let Some(local) = self.variable("LOCALAPPDATA") {
                values.push(PathBuf::from(local).join("Android/Sdk"));
            }
        } else if let Some(home) = home {
            values.push(home.join("Android/Sdk"));
            values.push(home.join("android-sdk"));
        }
        values
    }
}

#[derive(Clone, Debug, Default)]
pub struct Doctor;

impl Doctor {
    #[must_use]
    pub fn inspect(
        resolver: &SdkResolver,
        tools: Result<AndroidTools, SdkError>,
    ) -> DoctorSnapshot {
        let mut checks = Vec::new();
        if !resolver.conflicts().is_empty() {
            checks.push(DoctorCheck {
                code: "sdk.environment_conflict".into(),
                status: DoctorStatus::Warning,
                message: "ANDROID_SDK_ROOT and ANDROID_HOME resolve to different paths".into(),
                suggestion: Some("remove ANDROID_HOME or make both variables identical".into()),
            });
        }
        let Ok(tools) = tools else {
            checks.push(DoctorCheck {
                code: "sdk.missing".into(),
                status: DoctorStatus::Error,
                message: "Android SDK could not be resolved".into(),
                suggestion: None,
            });
            return DoctorSnapshot {
                tools: None,
                checks,
            };
        };
        check_tool(&mut checks, "adb", &tools.adb, Some("platform-tools"));
        check_tool(&mut checks, "emulator", &tools.emulator, Some("emulator"));
        check_tool(
            &mut checks,
            "sdkmanager",
            &tools.sdkmanager,
            Some("cmdline-tools;latest"),
        );
        check_tool(
            &mut checks,
            "avdmanager",
            &tools.avdmanager,
            Some("cmdline-tools;latest"),
        );
        check_tool(&mut checks, "java", &tools.java, None);
        DoctorSnapshot {
            tools: Some(tools),
            checks,
        }
    }
}

fn check_tool(checks: &mut Vec<DoctorCheck>, name: &str, path: &Path, package: Option<&str>) {
    let exists = path.is_file();
    checks.push(DoctorCheck {
        code: format!("tool.{name}"),
        status: if exists {
            DoctorStatus::Ok
        } else {
            DoctorStatus::Error
        },
        message: if exists {
            format!("{name} found at {}", path.display())
        } else {
            format!("{name} is missing at {}", path.display())
        },
        suggestion: (!exists)
            .then(|| package.map(|value| format!("sdkmanager \"{value}\"")))
            .flatten(),
    });
}

fn read_local_properties(path: &Path) -> Result<Option<PathBuf>, SdkError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SdkError::LocalProperties {
                path: path.into(),
                source,
            });
        }
    };
    Ok(source.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        (key.trim() == "sdk.dir")
            .then(|| PathBuf::from(value.trim().replace("\\:", ":").replace("\\\\", "\\")))
    }))
}

fn platform_executable(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        path.with_extension("exe")
    } else {
        path
    }
}

fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn push(values: &mut Vec<(PathBuf, SdkSource)>, path: Option<PathBuf>, source: SdkSource) {
    if let Some(path) = path.filter(|path| !path.as_os_str().is_empty()) {
        values.push((path, source));
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    #[error("Android SDK was not found; pass --sdk or configure android.sdk_path locally")]
    MissingSdk,
    #[error("Java was not found through JAVA_HOME or PATH")]
    MissingJava,
    #[error("Android SDK path {path:?} is invalid: {source}")]
    InvalidSdk {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Android SDK path {0:?} is not a directory")]
    InvalidLayout(PathBuf),
    #[error("failed to read {path:?}: {source}")]
    LocalProperties {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, [])
    }

    #[test]
    fn cli_precedes_configuration_local_properties_and_environment()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let cli = temp.path().join("cli");
        let config = temp.path().join("config");
        fs::create_dir_all(&cli)?;
        fs::create_dir_all(&config)?;
        let java = temp.path().join(if cfg!(windows) {
            "bin/java.exe"
        } else {
            "bin/java"
        });
        tool(&java)?;
        let mut environment = BTreeMap::new();
        environment.insert(OsString::from("JAVA_HOME"), temp.path().as_os_str().into());
        environment.insert(
            OsString::from("ANDROID_SDK_ROOT"),
            config.as_os_str().into(),
        );
        let tools = SdkResolver::with_environment(environment, None).resolve(&SdkResolution {
            cli: Some(cli.clone()),
            configuration: Some(config),
            ..SdkResolution::default()
        })?;
        assert_eq!(tools.sdk_root, cli.canonicalize()?);
        assert_eq!(tools.source, SdkSource::Cli);
        Ok(())
    }

    #[test]
    fn doctor_reports_exact_packages_without_installing() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let java = temp.path().join(if cfg!(windows) {
            "bin/java.exe"
        } else {
            "bin/java"
        });
        tool(&java)?;
        let mut environment = BTreeMap::new();
        environment.insert(OsString::from("JAVA_HOME"), temp.path().as_os_str().into());
        let resolver = SdkResolver::with_environment(environment, None);
        let snapshot = Doctor::inspect(
            &resolver,
            resolver.resolve(&SdkResolution {
                cli: Some(temp.path().into()),
                ..SdkResolution::default()
            }),
        );
        assert!(
            snapshot
                .checks
                .iter()
                .any(|check| check.suggestion.as_deref() == Some("sdkmanager \"platform-tools\""))
        );
        Ok(())
    }
}
