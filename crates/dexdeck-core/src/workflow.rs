use std::{future::Future, pin::Pin};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workflow {
    Build,
    Install,
    Launch,
    Run,
    Rerun,
    Reinstall,
    CleanReinstall,
    Stop,
    Uninstall,
    ClearData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowStep {
    Assemble,
    LocateOrAssembleArtifact,
    Install,
    Launch,
    ForceStop,
    Uninstall,
    ClearData,
}

impl WorkflowStep {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Assemble => "assemble",
            Self::LocateOrAssembleArtifact => "locate-or-assemble-artifact",
            Self::Install => "install",
            Self::Launch => "launch",
            Self::ForceStop => "force-stop",
            Self::Uninstall => "uninstall",
            Self::ClearData => "clear-data",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkflowRequest {
    pub workflow: Workflow,
    pub confirmed: bool,
    pub release_confirmation_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkflowEvent {
    Started {
        total_steps: u32,
    },
    StepStarted {
        index: u32,
        total: u32,
        step: WorkflowStep,
    },
    StepFinished {
        index: u32,
        total: u32,
        step: WorkflowStep,
    },
    Finished,
    Cancelled,
    Error {
        step: WorkflowStep,
        message: String,
    },
}

pub type WorkflowStepFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

pub trait WorkflowStepRunner: Send + Sync {
    fn run_step<'a>(
        &'a self,
        step: WorkflowStep,
        cancel: CancellationToken,
    ) -> WorkflowStepFuture<'a>;
}

#[derive(Clone, Debug, Default)]
pub struct WorkflowExecutor;

impl WorkflowExecutor {
    pub async fn execute<R: WorkflowStepRunner>(
        &self,
        request: &WorkflowRequest,
        runner: &R,
        events: &mpsc::Sender<WorkflowEvent>,
        cancel: CancellationToken,
    ) -> Result<(), WorkflowError> {
        if (destructive(request.workflow) || request.release_confirmation_required)
            && !request.confirmed
        {
            return Err(WorkflowError::ConfirmationRequired);
        }
        let steps = steps(request.workflow);
        let total = u32::try_from(steps.len()).unwrap_or(u32::MAX);
        send(events, WorkflowEvent::Started { total_steps: total }).await;
        for (offset, step) in steps.into_iter().enumerate() {
            if cancel.is_cancelled() {
                send(events, WorkflowEvent::Cancelled).await;
                return Err(WorkflowError::Cancelled);
            }
            let index = u32::try_from(offset.saturating_add(1)).unwrap_or(u32::MAX);
            send(events, WorkflowEvent::StepStarted { index, total, step }).await;
            let result = runner.run_step(step, cancel.child_token()).await;
            if cancel.is_cancelled() {
                send(events, WorkflowEvent::Cancelled).await;
                return Err(WorkflowError::Cancelled);
            }
            if let Err(message) = result {
                send(
                    events,
                    WorkflowEvent::Error {
                        step,
                        message: message.clone(),
                    },
                )
                .await;
                return Err(WorkflowError::Step { step, message });
            }
            send(events, WorkflowEvent::StepFinished { index, total, step }).await;
        }
        send(events, WorkflowEvent::Finished).await;
        Ok(())
    }
}

async fn send(events: &mpsc::Sender<WorkflowEvent>, event: WorkflowEvent) {
    let _ = events.send(event).await;
}

fn destructive(workflow: Workflow) -> bool {
    matches!(
        workflow,
        Workflow::CleanReinstall | Workflow::Uninstall | Workflow::ClearData
    )
}

fn steps(workflow: Workflow) -> Vec<WorkflowStep> {
    use WorkflowStep::{
        Assemble, ClearData, ForceStop, Install, Launch, LocateOrAssembleArtifact, Uninstall,
    };
    match workflow {
        Workflow::Build => vec![Assemble],
        Workflow::Install => vec![LocateOrAssembleArtifact, Install],
        Workflow::Launch => vec![Launch],
        Workflow::Run => vec![Assemble, Install, Launch],
        Workflow::Rerun => vec![ForceStop, Launch],
        Workflow::Reinstall => vec![Assemble, Install, Launch],
        Workflow::CleanReinstall => vec![Assemble, Uninstall, Install, Launch],
        Workflow::Stop => vec![ForceStop],
        Workflow::Uninstall => vec![Uninstall],
        Workflow::ClearData => vec![ClearData],
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("operation requires interactive confirmation or --yes")]
    ConfirmationRequired,
    #[error("workflow was cancelled")]
    Cancelled,
    #[error("workflow step {step:?} failed: {message}")]
    Step { step: WorkflowStep, message: String },
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<WorkflowStep>>);

    impl WorkflowStepRunner for Recorder {
        fn run_step<'a>(
            &'a self,
            step: WorkflowStep,
            _: CancellationToken,
        ) -> WorkflowStepFuture<'a> {
            Box::pin(async move {
                self.0.lock().map_err(|_| "poisoned".to_owned())?.push(step);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn run_and_clean_reinstall_have_explicit_stable_steps() {
        let runner = Recorder::default();
        let (events, mut receiver) = mpsc::channel(32);
        WorkflowExecutor
            .execute(
                &WorkflowRequest {
                    workflow: Workflow::Run,
                    confirmed: false,
                    release_confirmation_required: false,
                },
                &runner,
                &events,
                CancellationToken::new(),
            )
            .await
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            *runner.0.lock().unwrap_or_else(|error| error.into_inner()),
            [
                WorkflowStep::Assemble,
                WorkflowStep::Install,
                WorkflowStep::Launch
            ]
        );
        assert!(receiver.try_recv().is_ok());
        let rejected = WorkflowExecutor
            .execute(
                &WorkflowRequest {
                    workflow: Workflow::CleanReinstall,
                    confirmed: false,
                    release_confirmation_required: false,
                },
                &runner,
                &events,
                CancellationToken::new(),
            )
            .await;
        assert_eq!(rejected, Err(WorkflowError::ConfirmationRequired));
    }
}
