use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
};

use dexdeck_config::{RecoveredFile, StorageError, load_job_history, save_job_history};
use dexdeck_protocol::{Diagnostic, JobId, JobRecord, JobState};

use crate::SecretRedactor;

pub const JOB_HISTORY_LIMIT: usize = 50;
pub const DEFAULT_JOB_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputBuffer {
    bytes: VecDeque<u8>,
    capacity: usize,
    discarded_bytes: u64,
}

impl OutputBuffer {
    pub fn new(capacity: usize) -> Result<Self, JobSchedulerError> {
        if capacity == 0 {
            return Err(JobSchedulerError::InvalidOutputCapacity);
        }
        Ok(Self {
            bytes: VecDeque::with_capacity(capacity),
            capacity,
            discarded_bytes: 0,
        })
    }

    pub fn append(&mut self, bytes: &[u8]) {
        let overflow = self
            .bytes
            .len()
            .saturating_add(bytes.len())
            .saturating_sub(self.capacity);
        let discarded = overflow.min(self.bytes.len());
        self.bytes.drain(..discarded);

        let input_start = bytes.len().saturating_sub(self.capacity);
        self.discarded_bytes = self
            .discarded_bytes
            .saturating_add(overflow.try_into().unwrap_or(u64::MAX));
        self.bytes.extend(&bytes[input_start..]);
    }

    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.bytes.iter().copied().collect()
    }

    #[must_use]
    pub fn text_lossy(&self) -> String {
        String::from_utf8_lossy(&self.bytes()).into_owned()
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub const fn discarded_bytes(&self) -> u64 {
        self.discarded_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobRequest {
    pub record: JobRecord,
    /// Set only for operations that mutate a Gradle root.
    pub mutating_gradle_root: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobSchedule {
    StartNow,
    Queued,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationDirective {
    None,
    Graceful,
    Force,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobFinish {
    pub state: JobState,
    pub finished_at: String,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Job {
    record: JobRecord,
    mutating_gradle_root: Option<String>,
    output: OutputBuffer,
    cancellation_requests: u8,
}

impl Job {
    #[must_use]
    pub const fn record(&self) -> &JobRecord {
        &self.record
    }

    #[must_use]
    pub const fn output(&self) -> &OutputBuffer {
        &self.output
    }

    #[must_use]
    pub fn mutating_gradle_root(&self) -> Option<&str> {
        self.mutating_gradle_root.as_deref()
    }
}

#[derive(Debug)]
pub struct JobScheduler {
    jobs: HashMap<JobId, Job>,
    queued: VecDeque<JobId>,
    active_gradle_roots: HashSet<String>,
    history: VecDeque<JobRecord>,
    output_capacity: usize,
}

impl JobScheduler {
    pub fn new(output_capacity: usize) -> Result<Self, JobSchedulerError> {
        OutputBuffer::new(output_capacity)?;
        Ok(Self {
            jobs: HashMap::new(),
            queued: VecDeque::new(),
            active_gradle_roots: HashSet::new(),
            history: VecDeque::with_capacity(JOB_HISTORY_LIMIT),
            output_capacity,
        })
    }

    pub fn submit(&mut self, mut request: JobRequest) -> Result<JobSchedule, JobSchedulerError> {
        let id = request.record.id.clone();
        if self.jobs.contains_key(&id) {
            return Err(JobSchedulerError::DuplicateJob(id));
        }
        if is_terminal(request.record.state) {
            return Err(JobSchedulerError::InvalidTransition {
                id,
                from: request.record.state,
                to: JobState::Queued,
            });
        }

        let blocked = request
            .mutating_gradle_root
            .as_ref()
            .is_some_and(|root| self.active_gradle_roots.contains(root));
        let schedule = if blocked {
            request.record.state = JobState::Queued;
            self.queued.push_back(id.clone());
            JobSchedule::Queued
        } else {
            request.record.state = JobState::Starting;
            if let Some(root) = &request.mutating_gradle_root {
                self.active_gradle_roots.insert(root.clone());
            }
            JobSchedule::StartNow
        };

        self.jobs.insert(
            id,
            Job {
                record: request.record,
                mutating_gradle_root: request.mutating_gradle_root,
                output: OutputBuffer::new(self.output_capacity)?,
                cancellation_requests: 0,
            },
        );
        Ok(schedule)
    }

    pub fn mark_running(&mut self, id: &JobId) -> Result<(), JobSchedulerError> {
        self.transition(id, JobState::Starting, JobState::Running)
    }

    pub fn append_output(&mut self, id: &JobId, bytes: &[u8]) -> Result<(), JobSchedulerError> {
        let job = self.job_mut(id)?;
        if is_terminal(job.record.state) {
            return Err(JobSchedulerError::JobAlreadyFinished(id.clone()));
        }
        job.output.append(bytes);
        Ok(())
    }

    pub fn add_diagnostic(
        &mut self,
        id: &JobId,
        diagnostic: Diagnostic,
    ) -> Result<(), JobSchedulerError> {
        let job = self.job_mut(id)?;
        if is_terminal(job.record.state) {
            return Err(JobSchedulerError::JobAlreadyFinished(id.clone()));
        }
        job.record.diagnostics.push(diagnostic);
        Ok(())
    }

    pub fn cancel(&mut self, id: &JobId) -> Result<CancellationDirective, JobSchedulerError> {
        let state = self.job(id)?.record.state;
        match state {
            JobState::Queued => {
                self.queued.retain(|queued_id| queued_id != id);
                let job = self.job_mut(id)?;
                job.record.state = JobState::Cancelled;
                self.record_history(id)?;
                self.jobs.remove(id);
                Ok(CancellationDirective::None)
            }
            JobState::Starting | JobState::Running => {
                let job = self.job_mut(id)?;
                job.record.state = JobState::Cancelling;
                job.cancellation_requests = 1;
                Ok(CancellationDirective::Graceful)
            }
            JobState::Cancelling => {
                let job = self.job_mut(id)?;
                job.cancellation_requests = job.cancellation_requests.saturating_add(1);
                Ok(CancellationDirective::Force)
            }
            JobState::Succeeded | JobState::Failed | JobState::Cancelled => {
                Ok(CancellationDirective::None)
            }
        }
    }

    pub fn finish(
        &mut self,
        id: &JobId,
        finish: JobFinish,
    ) -> Result<Vec<JobId>, JobSchedulerError> {
        if !is_terminal(finish.state) {
            return Err(JobSchedulerError::InvalidTerminalState(finish.state));
        }
        let root = {
            let job = self.job_mut(id)?;
            if is_terminal(job.record.state) {
                return Err(JobSchedulerError::JobAlreadyFinished(id.clone()));
            }
            job.record.state = finish.state;
            job.record.finished_at = Some(finish.finished_at);
            job.record.duration_ms = Some(finish.duration_ms);
            job.record.exit_code = finish.exit_code;
            job.mutating_gradle_root.clone()
        };
        self.record_history(id)?;
        self.jobs.remove(id);

        let mut promoted = Vec::new();
        if let Some(root) = root {
            self.active_gradle_roots.remove(&root);
            if let Some(queued_id) = self.next_queued_for_root(&root) {
                self.queued.retain(|candidate| candidate != &queued_id);
                self.job_mut(&queued_id)?.record.state = JobState::Starting;
                self.active_gradle_roots.insert(root);
                promoted.push(queued_id);
            }
        }
        Ok(promoted)
    }

    pub fn job(&self, id: &JobId) -> Result<&Job, JobSchedulerError> {
        self.jobs
            .get(id)
            .ok_or_else(|| JobSchedulerError::UnknownJob(id.clone()))
    }

    #[must_use]
    pub fn history(&self) -> &VecDeque<JobRecord> {
        &self.history
    }

    pub fn restore_history(&mut self, records: impl IntoIterator<Item = JobRecord>) {
        self.history.clear();
        for record in records
            .into_iter()
            .filter(|record| is_terminal(record.state))
        {
            if self.history.len() == JOB_HISTORY_LIMIT {
                self.history.pop_front();
            }
            self.history.push_back(record);
        }
    }

    pub fn persist_history(
        &self,
        path: &Path,
        redactor: &SecretRedactor,
    ) -> Result<(), StorageError> {
        let records = self
            .history
            .iter()
            .cloned()
            .map(|mut record| {
                record.command_summary = redactor.redact_argv(&record.command_summary);
                record.started_at = redactor.redact_text(&record.started_at);
                record.finished_at = record.finished_at.map(|value| redactor.redact_text(&value));
                record.module = record.module.map(|value| redactor.redact_text(&value));
                record.variant = record.variant.map(|value| redactor.redact_text(&value));
                record.device = record.device.map(|value| redactor.redact_text(&value));
                for diagnostic in &mut record.diagnostics {
                    diagnostic.message = redactor.redact_text(&diagnostic.message);
                    diagnostic.raw_context = diagnostic
                        .raw_context
                        .take()
                        .map(|value| redactor.redact_text(&value));
                    diagnostic.suggested_action = diagnostic
                        .suggested_action
                        .take()
                        .map(|value| redactor.redact_text(&value));
                }
                record
            })
            .collect::<Vec<_>>();
        save_job_history(path, &records)
    }

    pub fn restore_persisted_history(
        &mut self,
        path: &Path,
    ) -> Result<RecoveredFile<Vec<JobRecord>>, StorageError> {
        let recovered = load_job_history(path)?;
        if let RecoveredFile::Loaded(records) = &recovered {
            self.restore_history(records.clone());
        }
        Ok(recovered)
    }

    fn job_mut(&mut self, id: &JobId) -> Result<&mut Job, JobSchedulerError> {
        self.jobs
            .get_mut(id)
            .ok_or_else(|| JobSchedulerError::UnknownJob(id.clone()))
    }

    fn transition(
        &mut self,
        id: &JobId,
        expected: JobState,
        target: JobState,
    ) -> Result<(), JobSchedulerError> {
        let job = self.job_mut(id)?;
        if job.record.state != expected {
            return Err(JobSchedulerError::InvalidTransition {
                id: id.clone(),
                from: job.record.state,
                to: target,
            });
        }
        job.record.state = target;
        Ok(())
    }

    fn record_history(&mut self, id: &JobId) -> Result<(), JobSchedulerError> {
        let record = self.job(id)?.record.clone();
        if self.history.len() == JOB_HISTORY_LIMIT {
            self.history.pop_front();
        }
        self.history.push_back(record);
        Ok(())
    }

    fn next_queued_for_root(&self, root: &str) -> Option<JobId> {
        self.queued.iter().find_map(|id| {
            self.jobs
                .get(id)
                .filter(|job| job.mutating_gradle_root.as_deref() == Some(root))
                .map(|_| id.clone())
        })
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new(DEFAULT_JOB_OUTPUT_BYTES).unwrap_or_else(|_| unreachable!("non-zero capacity"))
    }
}

fn is_terminal(state: JobState) -> bool {
    matches!(
        state,
        JobState::Succeeded | JobState::Failed | JobState::Cancelled
    )
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum JobSchedulerError {
    #[error("job output capacity must be greater than zero")]
    InvalidOutputCapacity,
    #[error("job {0:?} already exists")]
    DuplicateJob(JobId),
    #[error("job {0:?} does not exist")]
    UnknownJob(JobId),
    #[error("job {0:?} has already finished")]
    JobAlreadyFinished(JobId),
    #[error("invalid transition for {id:?}: {from:?} to {to:?}")]
    InvalidTransition {
        id: JobId,
        from: JobState,
        to: JobState,
    },
    #[error("{0:?} is not a terminal job state")]
    InvalidTerminalState(JobState),
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::JobKind;

    use super::*;
    use crate::SensitiveValue;

    fn request(id: usize, root: Option<&str>) -> JobRequest {
        JobRequest {
            record: JobRecord {
                id: JobId(format!("job-{id}")),
                kind: JobKind::Build,
                state: JobState::Queued,
                project_identity: "project".into(),
                module: None,
                variant: None,
                device: None,
                command_summary: vec!["./gradlew".into(), "assemble".into()],
                started_at: format!("time-{id}"),
                finished_at: None,
                duration_ms: None,
                exit_code: None,
                diagnostics: vec![],
            },
            mutating_gradle_root: root.map(str::to_owned),
        }
    }

    fn finish(state: JobState) -> JobFinish {
        JobFinish {
            state,
            finished_at: "later".into(),
            duration_ms: 10,
            exit_code: Some(0),
        }
    }

    #[test]
    fn serializes_mutating_gradle_jobs_per_root() -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = JobScheduler::default();
        assert_eq!(
            scheduler.submit(request(1, Some("root-a")))?,
            JobSchedule::StartNow
        );
        assert_eq!(
            scheduler.submit(request(2, Some("root-a")))?,
            JobSchedule::Queued
        );
        assert_eq!(
            scheduler.submit(request(3, Some("root-b")))?,
            JobSchedule::StartNow
        );
        assert_eq!(scheduler.submit(request(4, None))?, JobSchedule::StartNow);

        let promoted = scheduler.finish(&JobId("job-1".into()), finish(JobState::Succeeded))?;
        assert_eq!(promoted, vec![JobId("job-2".into())]);
        assert_eq!(
            scheduler.job(&JobId("job-2".into()))?.record.state,
            JobState::Starting
        );
        Ok(())
    }

    #[test]
    fn output_is_strictly_byte_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = JobScheduler::new(5)?;
        let id = JobId("job-1".into());
        scheduler.submit(request(1, None))?;
        scheduler.append_output(&id, b"abc")?;
        scheduler.append_output(&id, b"defgh")?;

        assert_eq!(scheduler.job(&id)?.output.bytes(), b"defgh");
        assert_eq!(scheduler.job(&id)?.output.discarded_bytes(), 3);
        Ok(())
    }

    #[test]
    fn cancellation_escalates_and_queued_cancellation_does_not_signal()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = JobScheduler::default();
        let running = JobId("job-1".into());
        let queued = JobId("job-2".into());
        scheduler.submit(request(1, Some("root")))?;
        scheduler.submit(request(2, Some("root")))?;

        assert_eq!(scheduler.cancel(&running)?, CancellationDirective::Graceful);
        assert_eq!(scheduler.cancel(&running)?, CancellationDirective::Force);
        assert_eq!(scheduler.cancel(&queued)?, CancellationDirective::None);
        assert!(matches!(
            scheduler.job(&queued),
            Err(JobSchedulerError::UnknownJob(_))
        ));
        Ok(())
    }

    #[test]
    fn history_keeps_only_fifty_lightweight_records() -> Result<(), Box<dyn std::error::Error>> {
        let mut scheduler = JobScheduler::default();
        for id in 0..55 {
            let job_id = JobId(format!("job-{id}"));
            scheduler.submit(request(id, None))?;
            scheduler.finish(&job_id, finish(JobState::Succeeded))?;
        }

        assert_eq!(scheduler.history().len(), JOB_HISTORY_LIMIT);
        assert_eq!(
            scheduler.history().front().map(|record| &record.id),
            Some(&JobId("job-5".into()))
        );
        assert_eq!(
            scheduler.history().back().map(|record| &record.id),
            Some(&JobId("job-54".into()))
        );
        assert!(scheduler.jobs.is_empty());
        Ok(())
    }

    #[test]
    fn persisted_history_is_redacted_and_restorable() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("jobs.json");
        let mut scheduler = JobScheduler::default();
        let mut job = request(1, None);
        job.record.command_summary.push("boundary-secret".into());
        scheduler.submit(job)?;
        scheduler.finish(&JobId("job-1".into()), finish(JobState::Succeeded))?;
        let mut redactor = SecretRedactor::new();
        redactor.register(&SensitiveValue::new("boundary-secret"));
        scheduler.persist_history(&path, &redactor)?;
        assert!(!std::fs::read_to_string(&path)?.contains("boundary-secret"));
        let mut restored = JobScheduler::default();
        assert!(matches!(
            restored.restore_persisted_history(&path)?,
            RecoveredFile::Loaded(_)
        ));
        assert_eq!(restored.history().len(), 1);
        Ok(())
    }
}
