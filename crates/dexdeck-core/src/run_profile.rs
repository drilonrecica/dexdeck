use std::{collections::BTreeMap, fmt};

use dexdeck_config::{EnvironmentValue, LaunchMode, ProfileConfig, ResolvedConfig};
use dexdeck_protocol::{
    AndroidDevice, AndroidModule, DeviceState, ModuleKind, ProjectModel, Variant,
};

use crate::{SecretRedactor, SensitiveValue};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunProfileSelection {
    pub profile: Option<String>,
    pub module: Option<String>,
    pub variant: Option<String>,
    pub device: Option<String>,
    pub gradle_arguments: Vec<String>,
    pub require_device: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchRequest {
    Launcher,
    Activity {
        activity: String,
    },
    DeepLink {
        uri: String,
        action: Option<String>,
        categories: Vec<String>,
        extras: BTreeMap<String, dexdeck_config::IntentExtra>,
    },
}

pub struct ResolvedRunProfile {
    pub profile_name: Option<String>,
    pub module: AndroidModule,
    pub variant: Variant,
    pub device: Option<AndroidDevice>,
    pub launch: LaunchRequest,
    pub gradle_arguments: Vec<String>,
    pub gradle_properties: BTreeMap<String, SensitiveValue>,
    pub environment: BTreeMap<String, SensitiveValue>,
    pub release_confirmation_required: bool,
}

impl fmt::Debug for ResolvedRunProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedRunProfile")
            .field("profile_name", &self.profile_name)
            .field("module", &self.module.path)
            .field("variant", &self.variant.name)
            .field("device", &self.device.as_ref().map(|value| &value.serial))
            .field("launch", &self.launch)
            .field("gradle_argument_count", &self.gradle_arguments.len())
            .field("gradle_property_keys", &self.gradle_properties.keys())
            .field("environment_keys", &self.environment.keys())
            .field(
                "release_confirmation_required",
                &self.release_confirmation_required,
            )
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RunProfileResolver;

impl RunProfileResolver {
    pub fn resolve(
        model: &ProjectModel,
        config: &ResolvedConfig,
        devices: &[AndroidDevice],
        selection: &RunProfileSelection,
        redactor: &mut SecretRedactor,
    ) -> Result<ResolvedRunProfile, RunProfileError> {
        let profile = selection
            .profile
            .as_ref()
            .map(|name| {
                config
                    .profiles
                    .get(name)
                    .ok_or_else(|| RunProfileError::UnknownProfile(name.clone()))
            })
            .transpose()?;
        let module_name = selection
            .module
            .as_deref()
            .or_else(|| profile.and_then(|value| value.module.as_deref()))
            .or(config.project.default_module.as_deref());
        let module = select_module(model, module_name)?;
        if module.kind != ModuleKind::Application {
            return Err(RunProfileError::LibraryModule(module.path.clone()));
        }
        let variant_name = selection
            .variant
            .as_deref()
            .or_else(|| profile.and_then(|value| value.variant.as_deref()))
            .or(config.project.default_variant.as_deref());
        let variant = select_variant(module, variant_name)?;
        if !variant.enabled {
            return Err(RunProfileError::DisabledVariant(variant.name.clone()));
        }
        if variant.application_id.as_deref().is_none_or(str::is_empty) {
            return Err(RunProfileError::MissingApplicationId(variant.name.clone()));
        }

        let selector = selection
            .device
            .as_deref()
            .or_else(|| profile.and_then(|value| value.device.as_deref()));
        let device = selector
            .map(|value| select_device(devices, value))
            .transpose()?;
        if selection.require_device && device.is_none() {
            return Err(RunProfileError::DeviceRequired);
        }
        if let Some(device) = device
            && device.state != DeviceState::Online
        {
            return Err(RunProfileError::DeviceNotOnline {
                serial: device.serial.clone(),
                state: device.state.clone(),
            });
        }

        let launch = resolve_launch(profile)?;
        let mut gradle_arguments = config.gradle_arguments.clone();
        if let Some(profile) = profile {
            gradle_arguments.extend(profile.gradle_arguments.clone());
        }
        gradle_arguments.extend(selection.gradle_arguments.clone());
        let gradle_properties =
            resolve_environment(profile.map(|value| &value.gradle_properties), redactor)?;
        let environment = resolve_environment(profile.map(|value| &value.environment), redactor)?;
        let release =
            variant.debuggable == Some(false) || variant.build_type.eq_ignore_ascii_case("release");
        let prior_release_intent = selection.profile.is_some() && release;

        Ok(ResolvedRunProfile {
            profile_name: selection.profile.clone(),
            module: module.clone(),
            variant: variant.clone(),
            device: device.cloned(),
            launch,
            gradle_arguments,
            gradle_properties,
            environment,
            release_confirmation_required: release && !prior_release_intent,
        })
    }
}

fn select_module<'a>(
    model: &'a ProjectModel,
    selected: Option<&str>,
) -> Result<&'a AndroidModule, RunProfileError> {
    if let Some(selected) = selected {
        return model
            .modules
            .iter()
            .find(|module| module.path == selected)
            .ok_or_else(|| RunProfileError::UnknownModule(selected.into()));
    }
    let values = model
        .modules
        .iter()
        .filter(|module| module.kind == ModuleKind::Application)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [module] => Ok(module),
        [] => Err(RunProfileError::NoApplicationModule),
        _ => Err(RunProfileError::AmbiguousModule),
    }
}

fn select_variant<'a>(
    module: &'a AndroidModule,
    selected: Option<&str>,
) -> Result<&'a Variant, RunProfileError> {
    if let Some(selected) = selected {
        return module
            .variants
            .iter()
            .find(|variant| variant.name == selected)
            .ok_or_else(|| RunProfileError::UnknownVariant(selected.into()));
    }
    let values = module
        .variants
        .iter()
        .filter(|variant| variant.enabled)
        .collect::<Vec<_>>();
    match values.as_slice() {
        [variant] => Ok(variant),
        [] => Err(RunProfileError::NoEnabledVariant),
        _ => Err(RunProfileError::AmbiguousVariant),
    }
}

fn select_device<'a>(
    devices: &'a [AndroidDevice],
    selector: &str,
) -> Result<&'a AndroidDevice, RunProfileError> {
    if let Some(device) = devices.iter().find(|device| device.serial == selector) {
        return Ok(device);
    }
    let lower = selector.to_ascii_lowercase();
    let values = devices
        .iter()
        .filter(|device| {
            [
                device.model.as_deref(),
                device.product.as_deref(),
                device.device.as_deref(),
                device.avd_name.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.to_ascii_lowercase() == lower)
        })
        .collect::<Vec<_>>();
    match values.as_slice() {
        [device] => Ok(device),
        [] => Err(RunProfileError::UnknownDevice(selector.into())),
        _ => Err(RunProfileError::AmbiguousDevice(selector.into())),
    }
}

fn resolve_launch(profile: Option<&ProfileConfig>) -> Result<LaunchRequest, RunProfileError> {
    let Some(profile) = profile else {
        return Ok(LaunchRequest::Launcher);
    };
    match profile.launch_mode.unwrap_or(LaunchMode::Launcher) {
        LaunchMode::Launcher => Ok(LaunchRequest::Launcher),
        LaunchMode::Activity => profile
            .activity
            .as_ref()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .map(|activity| LaunchRequest::Activity { activity })
            .ok_or_else(|| {
                RunProfileError::InvalidIntent(
                    "activity launch requires a non-empty activity".into(),
                )
            }),
        LaunchMode::DeepLink => {
            let uri = profile
                .deep_link
                .as_ref()
                .filter(|value| value.contains(':') && !value.chars().any(char::is_whitespace))
                .cloned()
                .ok_or_else(|| {
                    RunProfileError::InvalidIntent(
                        "deep-link launch requires a valid URI with a scheme".into(),
                    )
                })?;
            if profile
                .intent_categories
                .iter()
                .any(|value| value.trim().is_empty() || value.chars().any(char::is_whitespace))
            {
                return Err(RunProfileError::InvalidIntent(
                    "intent categories cannot be empty or contain whitespace".into(),
                ));
            }
            Ok(LaunchRequest::DeepLink {
                uri,
                action: profile.intent_action.clone(),
                categories: profile.intent_categories.clone(),
                extras: profile.intent_extras.clone(),
            })
        }
    }
}

fn resolve_environment(
    values: Option<&BTreeMap<String, EnvironmentValue>>,
    redactor: &mut SecretRedactor,
) -> Result<BTreeMap<String, SensitiveValue>, RunProfileError> {
    values
        .into_iter()
        .flatten()
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

#[derive(Debug, thiserror::Error)]
pub enum RunProfileError {
    #[error("run profile {0:?} does not exist")]
    UnknownProfile(String),
    #[error("application module selection is required because multiple modules are available")]
    AmbiguousModule,
    #[error("project has no application module")]
    NoApplicationModule,
    #[error("module {0:?} does not exist")]
    UnknownModule(String),
    #[error("module {0:?} is a library and cannot be run")]
    LibraryModule(String),
    #[error("variant selection is required because multiple enabled variants are available")]
    AmbiguousVariant,
    #[error("module has no enabled application variant")]
    NoEnabledVariant,
    #[error("variant {0:?} does not exist")]
    UnknownVariant(String),
    #[error("variant {0:?} is disabled")]
    DisabledVariant(String),
    #[error("variant {0:?} has no application ID")]
    MissingApplicationId(String),
    #[error("an online device must be selected for this operation")]
    DeviceRequired,
    #[error("device {0:?} is unavailable")]
    UnknownDevice(String),
    #[error("device selector {0:?} is ambiguous")]
    AmbiguousDevice(String),
    #[error("device {serial:?} is not online: {state:?}")]
    DeviceNotOnline { serial: String, state: DeviceState },
    #[error("invalid launch intent: {0}")]
    InvalidIntent(String),
    #[error(transparent)]
    Secret(#[from] crate::SecretError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use dexdeck_protocol::{BuildInfo, VariantTasks};
    use std::path::PathBuf;

    fn model() -> ProjectModel {
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
            artifacts: vec![],
            test_components: vec![],
        };
        ProjectModel {
            root: PathBuf::from("/project"),
            build: BuildInfo {
                root: PathBuf::from("/project"),
                gradle_version: "9".into(),
                agp_version: Some("9".into()),
                java_version: None,
                kotlin_plugin_version: None,
            },
            included_builds: vec![],
            modules: vec![AndroidModule {
                path: ":app".into(),
                build_id: "root".into(),
                project_directory: PathBuf::from("/project/app"),
                kind: ModuleKind::Application,
                namespace: None,
                compile_sdk: None,
                target_sdk: None,
                minimum_sdk: None,
                flavor_dimensions: vec![],
                product_flavors: vec![],
                build_types: vec![],
                variants: vec![variant],
            }],
            tasks: vec![],
            diagnostics: vec![],
        }
    }

    #[test]
    fn resolves_only_unique_application_choice_and_requires_device_when_requested() {
        let config = ResolvedConfig::default();
        let result = RunProfileResolver::resolve(
            &model(),
            &config,
            &[],
            &RunProfileSelection {
                require_device: true,
                ..RunProfileSelection::default()
            },
            &mut SecretRedactor::new(),
        );
        assert!(matches!(result, Err(RunProfileError::DeviceRequired)));
    }
}
