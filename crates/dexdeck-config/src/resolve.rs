use std::{collections::BTreeMap, path::PathBuf};

use dexdeck_protocol::LogPriority;

use crate::{
    CommandConfig, ConfigError, ConfigLayer, EditorConfig, GradleConfig, KeymapPreset, LogScope,
    LogcatConfig, ProfileConfig, ProjectConfig, UiConfig, UnicodeMode,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub project: ProjectConfig,
    pub gradle_arguments: Vec<String>,
    pub keymap: KeymapPreset,
    pub reduced_motion: bool,
    pub unicode: UnicodeMode,
    pub logcat_buffer_mib: u16,
    pub logcat_minimum_priority: LogPriority,
    pub logcat_default_scope: LogScope,
    pub editor_command: Option<Vec<String>>,
    pub profiles: BTreeMap<String, ProfileConfig>,
    pub commands: BTreeMap<String, CommandConfig>,
}

impl Default for ResolvedConfig {
    fn default() -> Self {
        Self {
            project: ProjectConfig::default(),
            gradle_arguments: Vec::new(),
            keymap: KeymapPreset::Default,
            reduced_motion: false,
            unicode: UnicodeMode::Auto,
            logcat_buffer_mib: 32,
            logcat_minimum_priority: LogPriority::Debug,
            logcat_default_scope: LogScope::Application,
            editor_command: None,
            profiles: BTreeMap::new(),
            commands: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ConfigResolver {
    resolved: ResolvedConfig,
}

impl ConfigResolver {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(
        &mut self,
        layer: &ConfigLayer,
        source_path: impl Into<PathBuf>,
    ) -> Result<&mut Self, ConfigError> {
        layer.validate(source_path.into())?;
        apply_project(&mut self.resolved.project, &layer.project);
        apply_gradle(&mut self.resolved, &layer.gradle);
        apply_ui(&mut self.resolved, &layer.ui);
        apply_logcat(&mut self.resolved, &layer.logcat);
        apply_editor(&mut self.resolved, &layer.editor);
        self.resolved.profiles.extend(layer.profiles.clone());
        self.resolved.commands.extend(layer.commands.clone());
        Ok(self)
    }

    #[must_use]
    pub fn finish(self) -> ResolvedConfig {
        self.resolved
    }
}

fn apply_project(resolved: &mut ProjectConfig, layer: &ProjectConfig) {
    if let Some(module) = &layer.default_module {
        resolved.default_module = Some(module.clone());
    }
    if let Some(variant) = &layer.default_variant {
        resolved.default_variant = Some(variant.clone());
    }
}

fn apply_gradle(resolved: &mut ResolvedConfig, layer: &GradleConfig) {
    if let Some(arguments) = &layer.arguments {
        resolved.gradle_arguments.clone_from(arguments);
    }
}

fn apply_ui(resolved: &mut ResolvedConfig, layer: &UiConfig) {
    if let Some(keymap) = layer.keymap {
        resolved.keymap = keymap;
    }
    if let Some(reduced_motion) = layer.reduced_motion {
        resolved.reduced_motion = reduced_motion;
    }
    if let Some(unicode) = layer.unicode {
        resolved.unicode = unicode;
    }
}

fn apply_logcat(resolved: &mut ResolvedConfig, layer: &LogcatConfig) {
    if let Some(buffer_mib) = layer.buffer_mib {
        resolved.logcat_buffer_mib = buffer_mib;
    }
    if let Some(priority) = layer.minimum_priority {
        resolved.logcat_minimum_priority = priority;
    }
    if let Some(scope) = layer.default_scope {
        resolved.logcat_default_scope = scope;
    }
}

fn apply_editor(resolved: &mut ResolvedConfig, layer: &EditorConfig) {
    if let Some(command) = &layer.command {
        resolved.editor_command = Some(command.clone());
    }
}
