use std::path::{Path, PathBuf};

use thiserror::Error;

pub const MODEL_TASK: &str = "dexdeckModel";
pub const OUTPUT_PROPERTY: &str = "dexdeck.output";
pub const JAR_PROPERTY: &str = "dexdeck.bridgeJar";
pub const BRIDGE_JAVA_VERSION: u8 = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterKind {
    Agp8,
    Agp9,
    Degraded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BridgeInvocation {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub output: PathBuf,
}

impl BridgeInvocation {
    pub fn new(wrapper: &Path, init_script: &Path, output: &Path) -> Self {
        Self {
            executable: wrapper.to_path_buf(),
            arguments: vec![
                "--init-script".into(),
                init_script.display().to_string(),
                format!(
                    "-D{JAR_PROPERTY}={}",
                    init_script
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join("dexdeck-bridge.jar")
                        .display()
                ),
                format!("-D{OUTPUT_PROPERTY}={}", output.display()),
                "--console=plain".into(),
                MODEL_TASK.into(),
            ],
            output: output.to_path_buf(),
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum BridgeError {
    #[error("Android Gradle Plugin version is missing")]
    MissingAgpVersion,
    #[error("invalid Android Gradle Plugin version: {0}")]
    InvalidAgpVersion(String),
    #[error("bridge output is incomplete")]
    IncompleteOutput,
}

pub fn select_adapter(version: Option<&str>) -> Result<AdapterKind, BridgeError> {
    let version = version.ok_or(BridgeError::MissingAgpVersion)?;
    let major = version
        .split('.')
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| BridgeError::InvalidAgpVersion(version.into()))?;
    Ok(match major {
        8 => AdapterKind::Agp8,
        9 => AdapterKind::Agp9,
        _ => AdapterKind::Degraded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_versioned_adapters() {
        assert_eq!(select_adapter(Some("8.0.2")), Ok(AdapterKind::Agp8));
        assert_eq!(select_adapter(Some("9.3.0-alpha01")), Ok(AdapterKind::Agp9));
        assert_eq!(select_adapter(Some("7.4.2")), Ok(AdapterKind::Degraded));
    }

    #[test]
    fn builds_direct_wrapper_invocation() {
        let invocation = BridgeInvocation::new(
            Path::new("./gradlew"),
            Path::new("bridge.gradle"),
            Path::new("model.jsonl"),
        );
        assert_eq!(invocation.arguments[0], "--init-script");
        assert!(
            invocation
                .arguments
                .iter()
                .any(|arg| arg == "--console=plain")
        );
    }
}
