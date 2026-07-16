use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use dexdeck_config::{
    ModelCacheBundle, ModelFingerprint, ModelInputWatcher, ProjectIdentity, RecoveredFile,
    StoragePaths, discover_model_inputs, fingerprint, fingerprint_for_model, load_fingerprint,
    load_model, load_model_bundle, save_model_bundle,
};
use dexdeck_core::SecretRedactor;
use dexdeck_protocol::{
    DegradedReason as ProtocolDegradedReason, ModelFreshness, ProjectModel, ProjectSupport,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::{
    AdapterKind, BridgeFailure, BridgeRunOutput, BridgeRunner, ProjectDiscovery, discover_project,
    embedded_bridge_hash, model_hash, select_adapter,
};

pub type BridgeFuture<'a> =
    Pin<Box<dyn Future<Output = Result<BridgeRunOutput, BridgeFailure>> + Send + 'a>>;

pub trait BridgeModelProvider: Send + Sync {
    fn run<'a>(
        &'a self,
        discovery: &'a ProjectDiscovery,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
        redactor: &'a SecretRedactor,
    ) -> BridgeFuture<'a>;
    fn version(&self) -> &str;
}

impl BridgeModelProvider for BridgeRunner {
    fn run<'a>(
        &'a self,
        discovery: &'a ProjectDiscovery,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
        redactor: &'a SecretRedactor,
    ) -> BridgeFuture<'a> {
        Box::pin(self.run(
            &discovery.root,
            discovery.wrapper.as_deref(),
            cancel,
            force_cancel,
            redactor,
        ))
    }
    fn version(&self) -> &str {
        embedded_bridge_hash()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedProjectModel {
    pub model: ProjectModel,
    pub fingerprint: ModelFingerprint,
}

pub trait ProjectModelCache: Send + Sync {
    fn load(&self, root: &Path) -> Result<Option<CachedProjectModel>, String>;
    fn save(&self, root: &Path, cached: &CachedProjectModel) -> Result<(), String>;
}

#[derive(Clone, Debug)]
pub struct FileProjectModelCache {
    storage: StoragePaths,
}

impl FileProjectModelCache {
    #[must_use]
    pub const fn new(storage: StoragePaths) -> Self {
        Self { storage }
    }
    fn paths(&self, root: &Path) -> Result<dexdeck_config::ProjectPaths, String> {
        ProjectIdentity::from_path(root)
            .map(|id| self.storage.project(&id))
            .map_err(|error| error.to_string())
    }
}

impl ProjectModelCache for FileProjectModelCache {
    fn load(&self, root: &Path) -> Result<Option<CachedProjectModel>, String> {
        let paths = self.paths(root)?;
        match load_model_bundle(&paths.cache.join("model-bundle.json"))
            .map_err(|error| error.to_string())?
        {
            RecoveredFile::Loaded(bundle) => {
                return Ok(Some(CachedProjectModel {
                    model: bundle.model,
                    fingerprint: bundle.fingerprint,
                }));
            }
            RecoveredFile::Corrupt { message, .. } => return Err(message),
            RecoveredFile::Missing => {}
        }
        let model = load_model(&paths.model).map_err(|error| error.to_string())?;
        let fingerprint =
            load_fingerprint(&paths.fingerprint).map_err(|error| error.to_string())?;
        match (model, fingerprint) {
            (Some(model), Some(fingerprint)) => Ok(Some(CachedProjectModel { model, fingerprint })),
            (None, None) => Ok(None),
            _ => Err("project model cache is incomplete or corrupt".into()),
        }
    }
    fn save(&self, root: &Path, cached: &CachedProjectModel) -> Result<(), String> {
        let paths = self.paths(root)?;
        save_model_bundle(
            &paths.cache.join("model-bundle.json"),
            &ModelCacheBundle {
                model: cached.model.clone(),
                fingerprint: cached.fingerprint.clone(),
            },
        )
        .map_err(|error| error.to_string())
    }
}

pub trait ModelInputRegistrar: Send + Sync {
    fn replace(&self, root: &Path, inputs: &[PathBuf]) -> Result<(), String>;
    fn changed(&self) -> Result<bool, String>;
}

#[derive(Debug, Default)]
pub struct NoopModelInputRegistrar;
impl ModelInputRegistrar for NoopModelInputRegistrar {
    fn replace(&self, _root: &Path, _inputs: &[PathBuf]) -> Result<(), String> {
        Ok(())
    }
    fn changed(&self) -> Result<bool, String> {
        Ok(false)
    }
}

#[derive(Debug, Default)]
pub struct WatchingModelInputRegistrar {
    watcher: Mutex<Option<ModelInputWatcher>>,
}

impl ModelInputRegistrar for WatchingModelInputRegistrar {
    fn replace(&self, root: &Path, inputs: &[PathBuf]) -> Result<(), String> {
        let mut watched = Vec::with_capacity(inputs.len() + 1);
        watched.push(root.to_path_buf());
        watched.extend_from_slice(inputs);
        let watcher = ModelInputWatcher::start(&watched).map_err(|error| error.to_string())?;
        *self
            .watcher
            .lock()
            .map_err(|_| "model watcher lock is poisoned")? = Some(watcher);
        Ok(())
    }

    fn changed(&self) -> Result<bool, String> {
        let watcher = self
            .watcher
            .lock()
            .map_err(|_| "model watcher lock is poisoned".to_owned())?;
        watcher.as_ref().map_or(Ok(false), |watcher| {
            watcher.drain_changed().map_err(|error| error.to_string())
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectModelState {
    pub model: Option<ProjectModel>,
    pub freshness: ModelFreshness,
    pub support: ProjectSupport,
    pub degraded_reason: Option<ProtocolDegradedReason>,
    pub generation: u64,
}

#[derive(Debug, Error)]
pub enum ModelServiceError {
    #[error(transparent)]
    Discovery(#[from] crate::DiscoveryError),
    #[error("model cache failed: {0}")]
    Cache(String),
    #[error("model input fingerprint failed: {0}")]
    Fingerprint(#[from] std::io::Error),
    #[error("model watcher failed: {0}")]
    Watcher(String),
    #[error("project model service lock is poisoned")]
    Poisoned,
    #[error(transparent)]
    Bridge(#[from] BridgeFailure),
    #[error("project model is not open")]
    NotOpen,
}

#[derive(Clone, Debug)]
struct OpenProject {
    discovery: ProjectDiscovery,
    inputs: Vec<PathBuf>,
}

pub struct ProjectModelService {
    bridge: Arc<dyn BridgeModelProvider>,
    cache: Arc<dyn ProjectModelCache>,
    registrar: Arc<dyn ModelInputRegistrar>,
    generation: AtomicU64,
    commit: Mutex<()>,
    open: Mutex<Option<OpenProject>>,
    state: Mutex<ProjectModelState>,
}

impl std::fmt::Debug for ProjectModelService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectModelService")
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl ProjectModelService {
    #[must_use]
    pub fn new(
        bridge: Arc<dyn BridgeModelProvider>,
        cache: Arc<dyn ProjectModelCache>,
        registrar: Arc<dyn ModelInputRegistrar>,
    ) -> Self {
        Self {
            bridge,
            cache,
            registrar,
            generation: AtomicU64::new(0),
            commit: Mutex::new(()),
            open: Mutex::new(None),
            state: Mutex::new(ProjectModelState {
                model: None,
                freshness: ModelFreshness::Provisional,
                support: ProjectSupport::Full,
                degraded_reason: None,
                generation: 0,
            }),
        }
    }

    pub fn open(
        &self,
        start: &Path,
        explicit: bool,
    ) -> Result<ProjectModelState, ModelServiceError> {
        let _commit = self
            .commit
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)?;
        let discovery = discover_project(start, explicit)?;
        let inputs = discover_model_inputs(&discovery.root)?;
        self.registrar
            .replace(&discovery.root, &inputs)
            .map_err(ModelServiceError::Watcher)?;
        let cached = self
            .cache
            .load(&discovery.root)
            .map_err(ModelServiceError::Cache)?;
        let current = fingerprint(&inputs, cached.as_ref().map(|value| &value.fingerprint))?;
        let cache_is_current = cached.as_ref().is_some_and(|cached| {
            cached.fingerprint.schema_version == dexdeck_protocol::CACHE_SCHEMA_VERSION
                && cached.fingerprint.bridge_version == self.bridge.version()
                && cached.fingerprint.inputs == current.inputs
                && model_hash(&cached.model).is_ok_and(|hash| hash == cached.fingerprint.model_hash)
        });
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let (support, degraded_reason) = cached
            .as_ref()
            .and_then(|value| value.model.build.agp_version.as_deref())
            .and_then(|version| {
                select_adapter(Some(version))
                    .ok()
                    .map(|adapter| (version, adapter))
            })
            .filter(|(_, adapter)| *adapter == AdapterKind::Degraded)
            .map_or_else(
                || {
                    if discovery.wrapper.is_none() {
                        (
                            ProjectSupport::Degraded,
                            Some(ProtocolDegradedReason::MissingWrapper),
                        )
                    } else {
                        (ProjectSupport::Full, None)
                    }
                },
                |(version, _)| {
                    (
                        ProjectSupport::Degraded,
                        Some(ProtocolDegradedReason::UnsupportedAgp {
                            detected: version.into(),
                            supported: "8.x-9.x".into(),
                        }),
                    )
                },
            );
        let state = ProjectModelState {
            model: cached.as_ref().map(|value| value.model.clone()),
            freshness: if cache_is_current {
                ModelFreshness::Current
            } else if cached.is_some() {
                ModelFreshness::Stale
            } else {
                ModelFreshness::Provisional
            },
            support,
            degraded_reason,
            generation,
        };
        *self.open.lock().map_err(|_| ModelServiceError::Poisoned)? =
            Some(OpenProject { discovery, inputs });
        *self.state.lock().map_err(|_| ModelServiceError::Poisoned)? = state.clone();
        Ok(state)
    }

    pub fn state(&self) -> Result<ProjectModelState, ModelServiceError> {
        self.state
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)
            .map(|state| state.clone())
    }

    pub async fn refresh(
        &self,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
        redactor: &SecretRedactor,
    ) -> Result<ProjectModelState, ModelServiceError> {
        let open = self
            .open
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)?
            .clone()
            .ok_or(ModelServiceError::NotOpen)?;
        let generation = {
            let _commit = self
                .commit
                .lock()
                .map_err(|_| ModelServiceError::Poisoned)?;
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            let mut state = self.state.lock().map_err(|_| ModelServiceError::Poisoned)?;
            state.freshness = ModelFreshness::Refreshing;
            state.generation = generation;
            generation
        };
        let output = match self
            .bridge
            .run(&open.discovery, cancel, force_cancel, redactor)
            .await
        {
            Ok(output) => output,
            Err(error) => {
                let reason = match &error {
                    BridgeFailure::Embedded(crate::EmbeddedBridgeError::WrapperRequired) => {
                        ProtocolDegradedReason::MissingWrapper
                    }
                    BridgeFailure::Reported { code, message } => {
                        ProtocolDegradedReason::BridgeFailed {
                            code: code.clone(),
                            message: message.clone(),
                        }
                    }
                    _ => ProtocolDegradedReason::BridgeFailed {
                        code: "bridge.failed".into(),
                        message: redactor.redact_text(&error.to_string()),
                    },
                };
                self.mark_degraded(generation, reason)?;
                return Err(error.into());
            }
        };
        let refreshed_inputs = discover_model_inputs(&open.discovery.root)?;
        let hash = model_hash(&output.model)
            .map_err(|error| ModelServiceError::Cache(error.to_string()))?;
        let previous = match self.cache.load(&open.discovery.root) {
            Ok(value) => value.map(|value| value.fingerprint),
            Err(error) => {
                self.mark_degraded(
                    generation,
                    ProtocolDegradedReason::CacheInvalid {
                        message: redactor.redact_text(&error),
                    },
                )?;
                return Err(ModelServiceError::Cache(error));
            }
        };
        let fingerprint = match fingerprint_for_model(
            &refreshed_inputs,
            previous.as_ref(),
            self.bridge.version(),
            hash,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.mark_degraded(
                    generation,
                    ProtocolDegradedReason::CacheInvalid {
                        message: redactor.redact_text(&error.to_string()),
                    },
                )?;
                return Err(error.into());
            }
        };
        let _commit = self
            .commit
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)?;
        if self.generation.load(Ordering::SeqCst) != generation {
            return self.state();
        }
        if let Err(error) = self
            .registrar
            .replace(&open.discovery.root, &refreshed_inputs)
        {
            drop(_commit);
            self.mark_degraded(
                generation,
                ProtocolDegradedReason::ConfigurationFailed {
                    message: redactor.redact_text(&error),
                },
            )?;
            return Err(ModelServiceError::Watcher(error));
        }
        if let Err(error) = self.cache.save(
            &open.discovery.root,
            &CachedProjectModel {
                model: output.model.clone(),
                fingerprint,
            },
        ) {
            drop(_commit);
            self.mark_degraded(
                generation,
                ProtocolDegradedReason::CacheInvalid {
                    message: redactor.redact_text(&error),
                },
            )?;
            return Err(ModelServiceError::Cache(error));
        }
        let mut state = self.state.lock().map_err(|_| ModelServiceError::Poisoned)?;
        if state.generation == generation {
            state.model = Some(output.model);
            state.freshness = ModelFreshness::Current;
            state.support = ProjectSupport::Full;
            state.degraded_reason = None;
            if let Some(open) = self
                .open
                .lock()
                .map_err(|_| ModelServiceError::Poisoned)?
                .as_mut()
            {
                open.inputs = refreshed_inputs;
            }
        }
        Ok(state.clone())
    }

    pub fn invalidate(&self) -> Result<ProjectModelState, ModelServiceError> {
        let _commit = self
            .commit
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)?;
        self.generation.fetch_add(1, Ordering::SeqCst);
        let mut state = self.state.lock().map_err(|_| ModelServiceError::Poisoned)?;
        state.freshness = if state.model.is_some() {
            ModelFreshness::Stale
        } else {
            ModelFreshness::Provisional
        };
        state.generation = self.generation.load(Ordering::SeqCst);
        Ok(state.clone())
    }

    pub fn poll_watcher(&self) -> Result<ProjectModelState, ModelServiceError> {
        if self
            .registrar
            .changed()
            .map_err(ModelServiceError::Watcher)?
        {
            self.invalidate()
        } else {
            self.state()
        }
    }

    fn mark_degraded(
        &self,
        generation: u64,
        reason: ProtocolDegradedReason,
    ) -> Result<(), ModelServiceError> {
        let _commit = self
            .commit
            .lock()
            .map_err(|_| ModelServiceError::Poisoned)?;
        if self.generation.load(Ordering::SeqCst) == generation {
            let mut state = self.state.lock().map_err(|_| ModelServiceError::Poisoned)?;
            state.freshness = ModelFreshness::Degraded;
            state.support = ProjectSupport::Degraded;
            state.degraded_reason = Some(reason);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio::sync::Notify;

    #[derive(Debug)]
    struct FailingBridge;
    impl BridgeModelProvider for FailingBridge {
        fn run<'a>(
            &'a self,
            _discovery: &'a ProjectDiscovery,
            _cancel: CancellationToken,
            _force_cancel: CancellationToken,
            _redactor: &'a SecretRedactor,
        ) -> BridgeFuture<'a> {
            Box::pin(async { Err(BridgeFailure::MissingOutput) })
        }
        fn version(&self) -> &str {
            "test-bridge"
        }
    }

    #[derive(Debug)]
    struct DelayedBridge {
        started: Arc<Notify>,
        release: Arc<Notify>,
        model: ProjectModel,
    }
    impl BridgeModelProvider for DelayedBridge {
        fn run<'a>(
            &'a self,
            _discovery: &'a ProjectDiscovery,
            _cancel: CancellationToken,
            _force_cancel: CancellationToken,
            _redactor: &'a SecretRedactor,
        ) -> BridgeFuture<'a> {
            Box::pin(async {
                self.started.notify_one();
                self.release.notified().await;
                Ok(BridgeRunOutput {
                    model: self.model.clone(),
                    stdout: String::new(),
                    stderr: String::new(),
                })
            })
        }
        fn version(&self) -> &str {
            "test-bridge"
        }
    }

    #[derive(Debug)]
    struct MemoryCache(Mutex<Option<CachedProjectModel>>);
    impl ProjectModelCache for MemoryCache {
        fn load(&self, _root: &Path) -> Result<Option<CachedProjectModel>, String> {
            self.0
                .lock()
                .map(|value| value.clone())
                .map_err(|_| "poisoned".into())
        }
        fn save(&self, _root: &Path, cached: &CachedProjectModel) -> Result<(), String> {
            self.0
                .lock()
                .map(|mut value| *value = Some(cached.clone()))
                .map_err(|_| "poisoned".into())
        }
    }

    #[tokio::test]
    async fn failed_refresh_preserves_stale_cached_model() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        std::fs::write(directory.path().join("settings.gradle"), "")?;
        std::fs::write(
            directory.path().join("build.gradle"),
            "plugins { id 'com.android.application' }",
        )?;
        let model = ProjectModel::empty(directory.path().canonicalize()?);
        let cache = Arc::new(MemoryCache(Mutex::new(Some(CachedProjectModel {
            model: model.clone(),
            fingerprint: ModelFingerprint {
                schema_version: dexdeck_protocol::CACHE_SCHEMA_VERSION,
                bridge_version: "old".into(),
                model_hash: "old".into(),
                inputs: Vec::new(),
            },
        }))));
        let service = ProjectModelService::new(
            Arc::new(FailingBridge),
            cache,
            Arc::new(NoopModelInputRegistrar),
        );
        let opened = service.open(directory.path(), true)?;
        assert_eq!(opened.model, Some(model.clone()));
        assert_eq!(opened.freshness, ModelFreshness::Stale);
        assert!(
            service
                .refresh(
                    CancellationToken::new(),
                    CancellationToken::new(),
                    &SecretRedactor::new()
                )
                .await
                .is_err()
        );
        let failed = service.state()?;
        assert_eq!(failed.model, Some(model));
        assert_eq!(failed.freshness, ModelFreshness::Degraded);
        Ok(())
    }

    #[tokio::test]
    async fn invalidated_refresh_cannot_replace_the_cache() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        std::fs::write(directory.path().join("settings.gradle"), "")?;
        std::fs::write(
            directory.path().join("build.gradle"),
            "plugins { id 'com.android.application' }",
        )?;
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let cache = Arc::new(MemoryCache(Mutex::new(None)));
        let service = Arc::new(ProjectModelService::new(
            Arc::new(DelayedBridge {
                started: Arc::clone(&started),
                release: Arc::clone(&release),
                model: ProjectModel::empty(directory.path().canonicalize()?),
            }),
            cache.clone(),
            Arc::new(NoopModelInputRegistrar),
        ));
        service.open(directory.path(), true)?;
        let refreshing = Arc::clone(&service);
        let task = tokio::spawn(async move {
            refreshing
                .refresh(
                    CancellationToken::new(),
                    CancellationToken::new(),
                    &SecretRedactor::new(),
                )
                .await
        });
        started.notified().await;
        let invalidated = service.invalidate()?;
        release.notify_one();
        let returned = task.await??;

        assert_eq!(returned.generation, invalidated.generation);
        assert!(cache.0.lock().map_err(|_| "poisoned")?.is_none());
        Ok(())
    }
}
