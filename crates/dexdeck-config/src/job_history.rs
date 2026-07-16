use std::path::Path;

use dexdeck_protocol::{JOB_HISTORY_SCHEMA_VERSION, JobRecord, JobState};

use crate::{
    RecoveredFile, StorageError, VersionedEnvelope, load_json_recovering, write_json_atomic,
};

pub const PERSISTED_JOB_HISTORY_LIMIT: usize = 50;

pub fn save_job_history(path: &Path, records: &[JobRecord]) -> Result<(), StorageError> {
    let terminal = records
        .iter()
        .filter(|record| is_terminal(record.state))
        .rev()
        .take(PERSISTED_JOB_HISTORY_LIMIT)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>();
    write_json_atomic(
        path,
        &VersionedEnvelope::new(JOB_HISTORY_SCHEMA_VERSION, terminal),
    )
}

pub fn load_job_history(path: &Path) -> Result<RecoveredFile<Vec<JobRecord>>, StorageError> {
    let loaded: RecoveredFile<Vec<JobRecord>> =
        load_json_recovering(path, JOB_HISTORY_SCHEMA_VERSION)?;
    Ok(match loaded {
        RecoveredFile::Loaded(records) => RecoveredFile::Loaded(
            records
                .into_iter()
                .filter(|record| is_terminal(record.state))
                .rev()
                .take(PERSISTED_JOB_HISTORY_LIMIT)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
        ),
        other => other,
    })
}

fn is_terminal(state: JobState) -> bool {
    matches!(
        state,
        JobState::Succeeded | JobState::Failed | JobState::Cancelled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dexdeck_protocol::{JobId, JobKind};

    fn record(index: usize, state: JobState) -> JobRecord {
        JobRecord {
            id: JobId(format!("job-{index}")),
            kind: JobKind::Build,
            state,
            project_identity: "project".into(),
            module: None,
            variant: None,
            device: None,
            command_summary: vec!["gradlew".into()],
            started_at: "now".into(),
            finished_at: Some("later".into()),
            duration_ms: Some(1),
            exit_code: Some(0),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn persists_only_latest_fifty_terminal_records() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("jobs.json");
        let mut records = (0..55)
            .map(|i| record(i, JobState::Succeeded))
            .collect::<Vec<_>>();
        records.push(record(56, JobState::Running));
        save_job_history(&path, &records)?;
        let RecoveredFile::Loaded(loaded) = load_job_history(&path)? else {
            panic!("history not loaded")
        };
        assert_eq!(loaded.len(), 50);
        assert_eq!(loaded.first().map(|r| r.id.0.as_str()), Some("job-5"));
        Ok(())
    }
}
