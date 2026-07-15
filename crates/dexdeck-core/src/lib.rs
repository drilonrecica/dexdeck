//! Application state and orchestration primitives for DexDeck.

mod app;
pub mod brand;
mod error;
mod job;
mod runtime;
mod secret;

pub use app::{
    Action, AppReducer, AppState, Effect, EffectOutcome, LifecycleState, ModelStatus, ProjectState,
    SubsystemState, SubsystemStatus, UiState,
};
pub use error::DexError;
pub use job::{
    CancellationDirective, DEFAULT_JOB_OUTPUT_BYTES, JOB_HISTORY_LIMIT, Job, JobFinish, JobRequest,
    JobSchedule, JobScheduler, JobSchedulerError, OutputBuffer,
};
pub use runtime::{
    ActionSender, AtomicIdGenerator, Clock, DEFAULT_ACTION_CAPACITY, DEFAULT_EFFECT_CAPACITY,
    DispatchError, EffectId, EffectRequest, IdGenerator, Reducer, Reduction, ReductionContext,
    Runtime, RuntimeConfig, RuntimeError, RuntimeParts, SystemClock,
};
pub use secret::{REDACTED, SecretError, SecretRedactor, SensitiveValue};
