//! Application state and orchestration primitives for DexDeck.

mod app;
pub mod brand;
mod custom_command;
mod debug_diagnostics;
mod error;
mod job;
mod log_buffer;
mod process;
mod run_profile;
mod runtime;
mod secret;
mod workflow;

pub use app::{
    Action, AppReducer, AppState, Effect, EffectOutcome, LifecycleState, ModelStatus, ProjectState,
    SubsystemState, SubsystemStatus, UiState,
};
pub use custom_command::{
    CommandPreview, CustomCommandError, CustomCommandService, TrustDecision, TrustFingerprint,
};
pub use debug_diagnostics::{
    DEFAULT_DEBUG_BYTES, DEFAULT_DEBUG_ENTRIES, DebugDiagnosticError, DebugDiagnostics, DebugEntry,
    DebugLevel, record_process_diagnostic, render_process_diagnostics,
};
pub use error::DexError;
pub use job::{
    CancellationDirective, DEFAULT_JOB_OUTPUT_BYTES, JOB_HISTORY_LIMIT, Job, JobFinish, JobRequest,
    JobSchedule, JobScheduler, JobSchedulerError, OutputBuffer,
};
pub use log_buffer::{
    ByteBoundedLogBuffer, DEFAULT_LOG_BUFFER_BYTES, LogBufferError, LogBufferStats,
    MAX_LOG_BUFFER_BYTES, MIN_LOG_BUFFER_BYTES, SequencedLogRecord,
};
#[cfg(windows)]
pub use process::run_windows_process_helper;
pub use process::{
    CommandSpec, ProcessError, ProcessResult, ProcessSupervisor, StreamingChild, TerminationReason,
};
pub use run_profile::{
    LaunchRequest, ResolvedRunProfile, RunProfileError, RunProfileResolver, RunProfileSelection,
};
pub use runtime::{
    ActionSender, AtomicIdGenerator, Clock, DEFAULT_ACTION_CAPACITY, DEFAULT_EFFECT_CAPACITY,
    DispatchError, EffectId, EffectRequest, IdGenerator, Reducer, Reduction, ReductionContext,
    Runtime, RuntimeConfig, RuntimeError, RuntimeParts, SystemClock,
};
pub use secret::{
    REDACTED, SecretError, SecretRedactor, SensitiveValue, StreamingSecretRedactor,
    resolve_environment_references,
};
pub use workflow::{
    Workflow, WorkflowError, WorkflowEvent, WorkflowExecutor, WorkflowRequest, WorkflowStep,
    WorkflowStepFuture, WorkflowStepRunner,
};
