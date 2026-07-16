use std::collections::VecDeque;

use dexdeck_protocol::{JobId, TestOutcome, TestRunResult, TestSelection};

pub const TEST_RUN_HISTORY_LIMIT: usize = 50;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedTestRun {
    pub job_id: JobId,
    pub task: String,
    pub arbitrary_task: bool,
    pub result: TestRunResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestRerunKind {
    All,
    Failed,
    Selected(TestSelection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestRerunPlan {
    pub source_job_id: JobId,
    pub task: String,
    pub selections: Vec<TestSelection>,
}

#[derive(Clone, Debug, Default)]
pub struct TestRunHistory {
    runs: VecDeque<RecordedTestRun>,
}

impl TestRunHistory {
    pub fn record(&mut self, run: RecordedTestRun) {
        if self.runs.len() == TEST_RUN_HISTORY_LIMIT {
            self.runs.pop_front();
        }
        self.runs.push_back(run);
    }

    #[must_use]
    pub fn latest(&self) -> Option<&RecordedTestRun> {
        self.runs.back()
    }

    pub fn plan(
        &self,
        job_id: Option<&JobId>,
        kind: TestRerunKind,
    ) -> Result<TestRerunPlan, TestRerunError> {
        let run = job_id.map_or_else(
            || self.latest().ok_or(TestRerunError::NoPreviousRun),
            |job_id| {
                self.runs
                    .iter()
                    .find(|run| &run.job_id == job_id)
                    .ok_or_else(|| TestRerunError::UnknownJob(job_id.clone()))
            },
        )?;
        let selections = match kind {
            TestRerunKind::All => vec![run.result.selection.clone()],
            TestRerunKind::Failed => {
                if run.arbitrary_task {
                    return Err(TestRerunError::ArbitraryTaskCannotReconstructFailures);
                }
                let selections = run
                    .result
                    .cases
                    .iter()
                    .filter(|case| case.outcome == TestOutcome::Failed)
                    .map(|case| TestSelection {
                        module: run.result.selection.module.clone(),
                        package: None,
                        class: Some(case.class.clone()),
                        method: Some(case.name.clone()),
                    })
                    .collect::<Vec<_>>();
                if selections.is_empty() {
                    return Err(TestRerunError::NoFailedTests);
                }
                selections
            }
            TestRerunKind::Selected(selection) => {
                if selection.method.is_some() && selection.class.is_none() {
                    return Err(TestRerunError::MethodWithoutClass);
                }
                let found = run.result.cases.iter().any(|case| {
                    selection
                        .class
                        .as_deref()
                        .is_none_or(|class| class == case.class)
                        && selection
                            .method
                            .as_deref()
                            .is_none_or(|method| method == case.name)
                });
                if !found {
                    return Err(TestRerunError::SelectionNotInRun);
                }
                vec![selection]
            }
        };
        Ok(TestRerunPlan {
            source_job_id: run.job_id.clone(),
            task: run.task.clone(),
            selections,
        })
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TestRerunError {
    #[error("there is no previous test run to rerun")]
    NoPreviousRun,
    #[error("test job {0:?} is not present in bounded history")]
    UnknownJob(JobId),
    #[error("the previous run has no failed tests")]
    NoFailedTests,
    #[error("failed selections cannot be reconstructed from an arbitrary test task; rerun all")]
    ArbitraryTaskCannotReconstructFailures,
    #[error("test method selection requires a class")]
    MethodWithoutClass,
    #[error("the selected class or method was not present in the source run")]
    SelectionNotInRun,
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::{TestCaseResult, TestRunSummary};

    use super::*;

    #[test]
    fn reconstructs_failed_methods_and_rejects_ambiguous_custom_tasks() -> Result<(), TestRerunError>
    {
        let result = TestRunResult {
            selection: TestSelection {
                module: Some(":app".into()),
                ..TestSelection::default()
            },
            summary: TestRunSummary {
                failed: 1,
                ..TestRunSummary::default()
            },
            cases: vec![TestCaseResult {
                suite: "suite".into(),
                class: "a.Example".into(),
                name: "fails".into(),
                outcome: TestOutcome::Failed,
                duration_ms: 1,
                failure_message: None,
                stack_trace: None,
                source: None,
            }],
        };
        let mut history = TestRunHistory::default();
        history.record(RecordedTestRun {
            job_id: JobId("job-1".into()),
            task: ":app:testDebugUnitTest".into(),
            arbitrary_task: false,
            result: result.clone(),
        });
        let plan = history.plan(None, TestRerunKind::Failed)?;
        assert_eq!(plan.selections[0].method.as_deref(), Some("fails"));
        history.record(RecordedTestRun {
            job_id: JobId("job-2".into()),
            task: "customTest".into(),
            arbitrary_task: true,
            result,
        });
        assert_eq!(
            history.plan(None, TestRerunKind::Failed),
            Err(TestRerunError::ArbitraryTaskCannotReconstructFailures)
        );
        Ok(())
    }
}
