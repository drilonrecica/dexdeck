//! Gradle discovery and bridge integration.

mod agp8;
mod agp9;
mod assembler;
mod bridge;
mod degraded;
mod discovery;
mod embedded;
mod refresh;
mod runner;
mod service;
mod task_runner;

pub use agp8::Agp8ModelBuilder;
pub use agp9::Agp9ModelBuilder;
pub use assembler::{ModelAssembler, ModelAssemblyError, model_hash};
pub use bridge::{
    AdapterKind, BRIDGE_JAVA_VERSION, BridgeError, BridgeInvocation, JAR_PROPERTY, MODEL_TASK,
    OUTPUT_PROPERTY, select_adapter,
};
pub use degraded::{DegradedCapabilities, DegradedMode, DegradedReason};
pub use discovery::{DiscoveryError, ProjectDiscovery, discover_project};
pub use embedded::{
    EmbeddedBridgeError, ExtractedBridge, embedded_bridge_hash, extract_bridge,
    parse_complete_output, select_gradle,
};
pub use refresh::{Freshness, ModelRefresh};
pub use runner::{BridgeFailure, BridgeRunOutput, BridgeRunner};
pub use service::{
    BridgeFuture, BridgeModelProvider, CachedProjectModel, FileProjectModelCache,
    ModelInputRegistrar, ModelServiceError, NoopModelInputRegistrar, ProjectModelCache,
    ProjectModelService, ProjectModelState, WatchingModelInputRegistrar,
};
pub use task_runner::{
    GradleArgumentLayers, GradleOutput, GradleOutputStream, GradleRunError, GradleRunRequest,
    GradleTaskRunner, validate_gradle_arguments,
};
