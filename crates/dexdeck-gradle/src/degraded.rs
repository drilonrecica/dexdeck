use dexdeck_protocol::ProjectModel;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DegradedReason {
    UnsupportedAgp {
        detected: String,
        supported: &'static str,
    },
    IncompatibleProtocol {
        expected: u32,
        found: u32,
    },
    ApiUnavailable {
        api: String,
    },
    MissingWrapper,
    ConfigurationFailed {
        message: String,
    },
}

impl DegradedReason {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::UnsupportedAgp {
                detected,
                supported,
            } => format!("AGP {detected} is outside the fully supported range {supported}"),
            Self::IncompatibleProtocol { expected, found } => {
                format!("bridge protocol {found} is incompatible; expected {expected}")
            }
            Self::ApiUnavailable { api } => {
                format!("required Android Gradle Plugin API is unavailable: {api}")
            }
            Self::MissingWrapper => {
                "the project has no Gradle wrapper and system Gradle was not approved".into()
            }
            Self::ConfigurationFailed { message } => {
                format!("Gradle project configuration failed: {message}")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DegradedCapabilities {
    pub cached_model: bool,
    pub gradle_tasks: bool,
    pub manual_profiles: bool,
    pub adb: bool,
    pub logcat: bool,
    pub full_model: bool,
}

#[derive(Clone, Debug)]
pub struct DegradedMode {
    pub reason: DegradedReason,
    pub cached_model: Option<ProjectModel>,
    pub capabilities: DegradedCapabilities,
}

impl DegradedMode {
    #[must_use]
    pub fn enter(
        reason: DegradedReason,
        cached_model: Option<ProjectModel>,
        wrapper_available: bool,
    ) -> Self {
        Self {
            capabilities: DegradedCapabilities {
                cached_model: cached_model.is_some(),
                gradle_tasks: wrapper_available,
                manual_profiles: true,
                adb: true,
                logcat: true,
                full_model: false,
            },
            reason,
            cached_model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn preserves_cache_without_claiming_full_support() {
        let mode = DegradedMode::enter(
            DegradedReason::UnsupportedAgp {
                detected: "7.4.2".into(),
                supported: "8.x-9.x",
            },
            Some(ProjectModel::empty(PathBuf::from("root"))),
            true,
        );
        assert!(mode.capabilities.cached_model);
        assert!(mode.capabilities.gradle_tasks);
        assert!(mode.capabilities.adb && mode.capabilities.logcat);
        assert!(!mode.capabilities.full_model);
    }

    #[test]
    fn missing_wrapper_disables_only_gradle_tasks() {
        let mode = DegradedMode::enter(DegradedReason::MissingWrapper, None, false);
        assert!(!mode.capabilities.gradle_tasks);
        assert!(mode.capabilities.manual_profiles);
    }
}
