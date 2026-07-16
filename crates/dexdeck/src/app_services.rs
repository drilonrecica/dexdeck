use std::{collections::HashMap, fmt, future::Future, path::PathBuf, pin::Pin, sync::Arc};

use dexdeck_core::{
    Action, ActionSender, AppReducer, AppState, AtomicIdGenerator, DEFAULT_ACTION_CAPACITY,
    DEFAULT_EFFECT_CAPACITY, Effect, EffectOutcome, Runtime, RuntimeConfig, ServiceKind,
    SystemClock,
};
use dexdeck_protocol::OperationError;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

pub type EffectFuture<'a> =
    Pin<Box<dyn Future<Output = Result<EffectOutcome, OperationError>> + Send + 'a>>;

pub trait EffectService: Send + Sync + 'static {
    fn execute<'a>(
        &'a self,
        effect: &'a Effect,
        cancellation: CancellationToken,
    ) -> EffectFuture<'a>;
}

#[derive(Default)]
pub struct ServiceRouter {
    services: HashMap<ServiceKind, Arc<dyn EffectService>>,
}

impl fmt::Debug for ServiceRouter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceRouter")
            .field("service_count", &self.services.len())
            .finish()
    }
}

impl ServiceRouter {
    #[must_use]
    pub fn with(mut self, kind: ServiceKind, service: Arc<dyn EffectService>) -> Self {
        self.services.insert(kind, service);
        self
    }

    fn service(&self, effect: &Effect) -> Option<(ServiceKind, &dyn EffectService)> {
        let kind = service_for_effect(effect);
        self.services
            .get(&kind)
            .map(|service| (kind, service.as_ref()))
    }
}

pub struct AppServices {
    actions: ActionSender<Action>,
    shutdown: CancellationToken,
    runtime: JoinHandle<()>,
    worker: JoinHandle<()>,
}

impl fmt::Debug for AppServices {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AppServices")
            .finish_non_exhaustive()
    }
}

impl AppServices {
    pub fn compose(
        project: Option<PathBuf>,
        router: ServiceRouter,
    ) -> Result<Self, dexdeck_core::RuntimeError> {
        let (runtime, actions, effects) = Runtime::new(
            AppState::default(),
            AppReducer,
            SystemClock,
            AtomicIdGenerator::default(),
            RuntimeConfig {
                action_capacity: DEFAULT_ACTION_CAPACITY,
                effect_capacity: DEFAULT_EFFECT_CAPACITY,
                completed_effect: Action::completed_effect_id,
                cancellation_target: Action::cancellation_target,
            },
        )?;
        let shutdown = CancellationToken::new();
        let runtime_handle = tokio::spawn(async move {
            let _ = runtime.run().await;
        });
        let worker_actions = actions.clone();
        let worker_shutdown = shutdown.clone();
        let worker = tokio::spawn(run_effects(
            effects,
            router,
            worker_actions,
            worker_shutdown,
        ));
        let _ = actions.try_dispatch(Action::StartRequested { project });
        Ok(Self {
            actions,
            shutdown,
            runtime: runtime_handle,
            worker,
        })
    }

    #[must_use]
    pub fn actions(&self) -> ActionSender<Action> {
        self.actions.clone()
    }

    pub async fn shutdown(self) {
        let _ = self.actions.dispatch(Action::ShutdownRequested).await;
        self.shutdown.cancel();
        let _ = self.worker.await;
        let _ = self.actions.dispatch(Action::ShutdownCompleted).await;
        drop(self.actions);
        let _ = self.runtime.await;
    }
}

async fn run_effects(
    mut effects: mpsc::Receiver<dexdeck_core::EffectRequest<Effect>>,
    router: ServiceRouter,
    actions: ActionSender<Action>,
    shutdown: CancellationToken,
) {
    loop {
        let request = tokio::select! {
            () = shutdown.cancelled() => break,
            request = effects.recv() => match request {
                Some(request) => request,
                None => break,
            }
        };
        let Some((kind, service)) = router.service(&request.effect) else {
            continue;
        };
        let outcome = tokio::select! {
            () = request.cancellation.cancelled() => EffectOutcome::Cancelled,
            result = service.execute(&request.effect, request.cancellation.clone()) => {
                match result {
                    Ok(outcome) => outcome,
                    Err(error) => {
                        let _ = actions.dispatch(Action::WorkerFailed {
                            service: kind,
                            error: error.clone(),
                        }).await;
                        EffectOutcome::Failed(error)
                    }
                }
            }
        };
        let _ = actions
            .dispatch(Action::EffectFinished {
                effect_id: request.id,
                outcome: Box::new(outcome),
            })
            .await;
    }
}

#[must_use]
pub const fn service_for_effect(effect: &Effect) -> ServiceKind {
    match effect {
        Effect::WriteSharedConfig | Effect::WriteLocalConfig => ServiceKind::Config,
        Effect::DiscoverProject { .. }
        | Effect::LoadProjectCache { .. }
        | Effect::RefreshProjectModel { .. }
        | Effect::WatchProjectFiles { .. } => ServiceKind::ProjectModel,
        Effect::StartGradleJob { .. } => ServiceKind::Jobs,
        Effect::RunTests { .. } => ServiceKind::Tests,
        Effect::OpenSourceLocation { .. } => ServiceKind::Diagnostics,
        Effect::StartLogcat { .. } | Effect::StopLogcat | Effect::ExportLogs { .. } => {
            ServiceKind::Logcat
        }
        Effect::TrackDevices
        | Effect::StartEmulator { .. }
        | Effect::StopEmulator { .. }
        | Effect::InstallArtifact { .. }
        | Effect::LaunchApplication { .. }
        | Effect::StopApplication { .. } => ServiceKind::Android,
    }
}
