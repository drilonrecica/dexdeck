//! Versioned internal and external data contracts.

mod bridge;
mod cli;
mod diagnostic;
mod job;
mod log;
mod model;
mod test_result;

pub use bridge::{
    BridgeComplete, BridgeEnvelope, BridgePayload, BridgeProtocolError, BridgeStreamValidator,
};
pub use cli::{CliEnvelope, CliEvent, OperationError, ProjectSnapshot};
pub use diagnostic::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, SourceLocation};
pub use job::{JobId, JobKind, JobRecord, JobState};
pub use log::{LogPriority, LogRecord};
pub use model::{
    AndroidModule, Artifact, ArtifactKind, BuildInfo, BuildType, FlavorDimension, GradleTask,
    IncludedBuild, LaunchComponent, ModuleKind, ProductFlavor, ProjectModel, TaskKind,
    TestComponent, TestComponentKind, Variant, VariantFlavor, VariantTasks,
};
pub use test_result::{TestCaseResult, TestOutcome, TestRunResult, TestRunSummary, TestSelection};

pub const CLI_SCHEMA_VERSION: u32 = 1;
pub const BRIDGE_PROTOCOL_VERSION: u32 = 1;
pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const CACHE_SCHEMA_VERSION: u32 = 1;
