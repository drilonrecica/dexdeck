use std::path::PathBuf;

use dexdeck_protocol::{OperationError, ProjectModel};

use crate::{EffectId, Reducer, Reduction, ReductionContext};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LifecycleState {
    #[default]
    Idle,
    Starting,
    Ready,
    ShuttingDown,
    Stopped,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ModelStatus {
    #[default]
    Unavailable,
    Discovering,
    Current,
    Stale,
    Failed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectState {
    pub root: Option<PathBuf>,
    pub model: Option<ProjectModel>,
    pub model_status: ModelStatus,
    pub last_error: Option<OperationError>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SubsystemStatus {
    #[default]
    Inactive,
    Starting,
    Active,
    Failed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SubsystemState {
    pub status: SubsystemStatus,
    pub last_error: Option<OperationError>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UiState {
    pub selected_module: Option<String>,
    pub selected_variant: Option<String>,
    pub selected_device: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppState {
    pub lifecycle: LifecycleState,
    pub project: ProjectState,
    pub devices: SubsystemState,
    pub emulator: SubsystemState,
    pub logcat: SubsystemState,
    pub jobs: SubsystemState,
    pub tests: SubsystemState,
    pub ui: UiState,
    pub revision: u64,
    pub updated_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    StartRequested {
        project: Option<PathBuf>,
    },
    EffectFinished {
        effect_id: EffectId,
        outcome: Box<EffectOutcome>,
    },
    CancelEffectRequested {
        effect_id: EffectId,
    },
    ShutdownRequested,
    ShutdownCompleted,
}

impl Action {
    #[must_use]
    pub fn completed_effect_id(&self) -> Option<EffectId> {
        match self {
            Self::EffectFinished { effect_id, .. } => Some(*effect_id),
            _ => None,
        }
    }

    #[must_use]
    pub fn cancellation_target(&self) -> Option<EffectId> {
        match self {
            Self::CancelEffectRequested { effect_id } => Some(*effect_id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    DiscoverProject {
        hint: Option<PathBuf>,
    },
    LoadProjectCache {
        root: PathBuf,
    },
    RefreshProjectModel {
        root: PathBuf,
    },
    WatchProjectFiles {
        root: PathBuf,
    },
    StartGradleJob {
        arguments: Vec<String>,
    },
    TrackDevices,
    StartLogcat {
        serial: String,
    },
    StopLogcat,
    StartEmulator {
        name: String,
        cold_boot: bool,
    },
    StopEmulator {
        serial: String,
    },
    InstallArtifact {
        serial: String,
        artifact: PathBuf,
    },
    LaunchApplication {
        serial: String,
        application_id: String,
    },
    StopApplication {
        serial: String,
        application_id: String,
    },
    RunTests {
        arguments: Vec<String>,
    },
    OpenSourceLocation {
        path: PathBuf,
        line: Option<u32>,
    },
    WriteSharedConfig,
    WriteLocalConfig,
    ExportLogs {
        destination: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectOutcome {
    Completed,
    ProjectDetected { root: PathBuf },
    ProjectModelLoaded { model: ProjectModel },
    Failed(OperationError),
    Cancelled,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AppReducer;

impl Reducer<AppState, Action, Effect> for AppReducer {
    fn reduce(
        &self,
        state: &AppState,
        action: &Action,
        context: ReductionContext,
    ) -> Reduction<AppState, Effect> {
        let mut next = state.clone();
        let mut effects = Vec::new();

        match action {
            Action::StartRequested { project } if state.lifecycle == LifecycleState::Idle => {
                next.lifecycle = LifecycleState::Starting;
                next.project.root.clone_from(project);
                next.project.model_status = ModelStatus::Discovering;
                effects.push(Effect::DiscoverProject {
                    hint: project.clone(),
                });
            }
            Action::EffectFinished { outcome, .. } => match outcome.as_ref() {
                EffectOutcome::ProjectDetected { root } => {
                    next.project.root = Some(root.clone());
                    effects.push(Effect::LoadProjectCache { root: root.clone() });
                    effects.push(Effect::RefreshProjectModel { root: root.clone() });
                }
                EffectOutcome::ProjectModelLoaded { model } => {
                    next.project.model = Some(model.clone());
                    next.project.model_status = ModelStatus::Current;
                    next.project.last_error = None;
                    next.lifecycle = LifecycleState::Ready;
                }
                EffectOutcome::Failed(error) => {
                    next.project.last_error = Some(error.clone());
                    next.project.model_status = ModelStatus::Failed;
                }
                EffectOutcome::Completed | EffectOutcome::Cancelled => {}
            },
            Action::ShutdownRequested if state.lifecycle != LifecycleState::Stopped => {
                next.lifecycle = LifecycleState::ShuttingDown;
            }
            Action::ShutdownCompleted => next.lifecycle = LifecycleState::Stopped,
            Action::StartRequested { .. }
            | Action::CancelEffectRequested { .. }
            | Action::ShutdownRequested => {}
        }

        if next != *state || !effects.is_empty() {
            next.revision = state.revision.saturating_add(1);
            next.updated_at_ms = context.now_ms;
        }

        Reduction::new(next, effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducer_is_pure_and_deterministic() {
        let state = AppState::default();
        let action = Action::StartRequested {
            project: Some(PathBuf::from("/workspace")),
        };
        let context = ReductionContext { now_ms: 42 };

        let first = AppReducer.reduce(&state, &action, context);
        let second = AppReducer.reduce(&state, &action, context);

        assert_eq!(first, second);
        assert_eq!(state, AppState::default());
        assert_eq!(first.state.lifecycle, LifecycleState::Starting);
        assert_eq!(first.state.revision, 1);
        assert_eq!(first.state.updated_at_ms, 42);
        assert_eq!(
            first.effects,
            vec![Effect::DiscoverProject {
                hint: Some(PathBuf::from("/workspace"))
            }]
        );
    }

    #[test]
    fn repeated_start_is_ignored() {
        let state = AppState {
            lifecycle: LifecycleState::Starting,
            revision: 3,
            ..AppState::default()
        };

        let reduction = AppReducer.reduce(
            &state,
            &Action::StartRequested { project: None },
            ReductionContext { now_ms: 99 },
        );

        assert_eq!(reduction.state, state);
        assert!(reduction.effects.is_empty());
    }
}
