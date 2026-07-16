use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use dexdeck_protocol::ProjectModel;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

pub const DEFAULT_MODEL_DEBOUNCE: Duration = Duration::from_millis(350);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionSelection {
    pub module: Option<String>,
    pub variant: Option<String>,
}

impl SessionSelection {
    #[must_use]
    pub fn restored(self, model: &ProjectModel) -> Self {
        let module = self
            .module
            .filter(|selected| model.modules.iter().any(|item| &item.path == selected));
        let variant = module.as_ref().and_then(|module_path| {
            let selected = self.variant?;
            model
                .modules
                .iter()
                .find(|item| &item.path == module_path)
                .and_then(|item| {
                    item.variants
                        .iter()
                        .any(|variant| variant.name == selected)
                        .then_some(selected)
                })
        });
        Self { module, variant }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchDecision {
    Idle,
    Stale,
    Refresh,
    DelayedForGradle,
}

#[derive(Debug)]
pub struct ModelWatchState {
    debounce: Duration,
    changed_at: Option<Instant>,
}

impl ModelWatchState {
    #[must_use]
    pub const fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            changed_at: None,
        }
    }

    pub fn mark_changed(&mut self, now: Instant) -> WatchDecision {
        self.changed_at = Some(now);
        WatchDecision::Stale
    }

    pub fn poll(&mut self, now: Instant, gradle_busy: bool) -> WatchDecision {
        let Some(changed_at) = self.changed_at else {
            return WatchDecision::Idle;
        };
        if now.saturating_duration_since(changed_at) < self.debounce {
            return WatchDecision::Stale;
        }
        if gradle_busy {
            return WatchDecision::DelayedForGradle;
        }
        self.changed_at = None;
        WatchDecision::Refresh
    }
}

#[derive(Debug, Error)]
pub enum ModelWatchError {
    #[error("failed to watch model inputs: {0}")]
    Notify(#[from] notify::Error),
}

#[derive(Debug)]
pub struct ModelInputWatcher {
    _watcher: RecommendedWatcher,
    receiver: Receiver<notify::Result<Event>>,
}

impl ModelInputWatcher {
    pub fn start(paths: &[PathBuf]) -> Result<Self, ModelWatchError> {
        let (sender, receiver) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = sender.send(event);
        })?;
        for path in paths {
            watcher.watch(
                path,
                if path.is_dir() {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )?;
        }
        Ok(Self {
            _watcher: watcher,
            receiver,
        })
    }

    pub fn drain_changed(&self) -> Result<bool, ModelWatchError> {
        let mut changed = false;
        while let Ok(event) = self.receiver.try_recv() {
            event?;
            changed = true;
        }
        Ok(changed)
    }
}

#[must_use]
pub fn is_model_input(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        let text = relative.to_string_lossy();
        matches!(
            text.as_ref(),
            "settings.gradle"
                | "settings.gradle.kts"
                | "build.gradle"
                | "build.gradle.kts"
                | "gradle.properties"
                | "gradle/libs.versions.toml"
                | "gradle/wrapper/gradle-wrapper.properties"
                | ".dexdeck/config.toml"
        ) || text.starts_with("buildSrc/")
            || text.starts_with("build-logic/")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dexdeck_protocol::{AndroidModule, BuildInfo, ModuleKind};

    #[test]
    fn marks_stale_immediately_and_waits_for_gradle() {
        let now = Instant::now();
        let mut state = ModelWatchState::new(Duration::from_millis(10));
        assert_eq!(state.mark_changed(now), WatchDecision::Stale);
        assert_eq!(
            state.poll(now + Duration::from_millis(20), true),
            WatchDecision::DelayedForGradle
        );
        assert_eq!(
            state.poll(now + Duration::from_millis(20), false),
            WatchDecision::Refresh
        );
    }

    #[test]
    fn drops_invalid_restored_selections() {
        let root = PathBuf::from("root");
        let model = ProjectModel {
            root: root.clone(),
            build: BuildInfo {
                root,
                gradle_version: "8.13".into(),
                agp_version: Some("8.13".into()),
                java_version: None,
                kotlin_plugin_version: None,
            },
            included_builds: vec![],
            modules: vec![AndroidModule {
                path: ":app".into(),
                build_id: "main".into(),
                kind: ModuleKind::Application,
                namespace: None,
                compile_sdk: None,
                target_sdk: None,
                minimum_sdk: None,
                flavor_dimensions: vec![],
                product_flavors: vec![],
                build_types: vec![],
                variants: vec![],
            }],
            tasks: vec![],
            diagnostics: vec![],
        };
        let restored = SessionSelection {
            module: Some(":missing".into()),
            variant: Some("debug".into()),
        }
        .restored(&model);
        assert_eq!(restored, SessionSelection::default());
    }
}
