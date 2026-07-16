use std::{
    fs,
    path::{Path, PathBuf},
};

use dexdeck_protocol::{
    SourceLocation, TestCaseResult, TestOutcome, TestRunResult, TestRunSummary, TestSelection,
};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReportWarning {
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTestReports {
    pub result: TestRunResult,
    pub warnings: Vec<TestReportWarning>,
}

#[derive(Clone, Debug, Default)]
pub struct TestReportParser;

impl TestReportParser {
    #[must_use]
    pub fn parse_junit_paths(paths: &[PathBuf], selection: TestSelection) -> ParsedTestReports {
        let mut cases = Vec::new();
        let mut warnings = Vec::new();
        for path in paths {
            let mut files = Vec::new();
            collect_xml(path, &mut files, &mut warnings);
            for file in files {
                match fs::read_to_string(&file)
                    .map_err(|error| error.to_string())
                    .and_then(|source| parse_junit(&source).map_err(|error| error.to_string()))
                {
                    Ok(mut parsed) => cases.append(&mut parsed),
                    Err(message) => warnings.push(TestReportWarning {
                        path: Some(file),
                        message,
                    }),
                }
            }
        }
        ParsedTestReports {
            result: summarize(selection, cases),
            warnings,
        }
    }

    #[must_use]
    pub fn parse_instrumentation(output: &str, selection: TestSelection) -> ParsedTestReports {
        let mut cases = Vec::new();
        let mut class = None::<String>;
        let mut test = None::<String>;
        let mut stack = None::<String>;
        let mut duration_ms = 0;
        let mut warnings = Vec::new();
        for line in output.lines() {
            if let Some(value) = line.strip_prefix("INSTRUMENTATION_STATUS: class=") {
                class = Some(value.trim().to_owned());
            } else if let Some(value) = line.strip_prefix("INSTRUMENTATION_STATUS: test=") {
                test = Some(value.trim().to_owned());
            } else if let Some(value) = line.strip_prefix("INSTRUMENTATION_STATUS: stack=") {
                stack = Some(value.to_owned());
            } else if let Some(value) = line.strip_prefix("INSTRUMENTATION_STATUS: time=") {
                duration_ms = seconds_to_ms(value);
            } else if let Some(value) = line.strip_prefix("INSTRUMENTATION_STATUS_CODE:") {
                let code = value.trim().parse::<i32>().ok();
                if code == Some(1) {
                    continue;
                }
                let (Some(class), Some(name)) = (class.take(), test.take()) else {
                    warnings.push(TestReportWarning {
                        path: None,
                        message: "instrumentation result lacked class or test name".into(),
                    });
                    continue;
                };
                let outcome = match code {
                    Some(0) => TestOutcome::Passed,
                    Some(-3) => TestOutcome::Skipped,
                    _ => TestOutcome::Failed,
                };
                let failure_message = (outcome == TestOutcome::Failed).then(|| {
                    stack
                        .as_deref()
                        .and_then(|value| value.lines().next())
                        .unwrap_or("instrumentation failure")
                        .to_owned()
                });
                let stack_trace = stack.take();
                cases.push(TestCaseResult {
                    suite: class.clone(),
                    class,
                    name,
                    outcome,
                    duration_ms,
                    failure_message,
                    source: stack_trace.as_deref().and_then(source_from_stack),
                    stack_trace,
                });
                duration_ms = 0;
            }
        }
        ParsedTestReports {
            result: summarize(selection, cases),
            warnings,
        }
    }
}

fn collect_xml(path: &Path, files: &mut Vec<PathBuf>, warnings: &mut Vec<TestReportWarning>) {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "xml") {
            files.push(path.to_path_buf());
        }
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(TestReportWarning {
                path: Some(path.to_path_buf()),
                message: error.to_string(),
            });
            return;
        }
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if child.is_dir() {
            collect_xml(&child, files, warnings);
        } else if child
            .extension()
            .is_some_and(|extension| extension == "xml")
        {
            files.push(child);
        }
    }
}

#[derive(Debug, Deserialize)]
struct XmlSuite {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "testcase", default)]
    cases: Vec<XmlCase>,
}

#[derive(Debug, Deserialize)]
struct XmlSuites {
    #[serde(rename = "testsuite", default)]
    suites: Vec<XmlSuite>,
}

#[derive(Debug, Deserialize)]
struct XmlCase {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "@classname", default)]
    class: String,
    #[serde(rename = "@time", default)]
    time: String,
    failure: Option<XmlFailure>,
    error: Option<XmlFailure>,
    skipped: Option<XmlSkipped>,
}

#[derive(Debug, Deserialize)]
struct XmlFailure {
    #[serde(rename = "@message")]
    message: Option<String>,
    #[serde(rename = "$text")]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XmlSkipped {}

fn parse_junit(source: &str) -> Result<Vec<TestCaseResult>, quick_xml::DeError> {
    let suites = match quick_xml::de::from_str::<XmlSuites>(source) {
        Ok(value) if !value.suites.is_empty() => value.suites,
        _ => vec![quick_xml::de::from_str::<XmlSuite>(source)?],
    };
    Ok(suites
        .into_iter()
        .flat_map(|suite| {
            suite.cases.into_iter().map(move |case| {
                let failure = case.failure.or(case.error);
                let outcome = if failure.is_some() {
                    TestOutcome::Failed
                } else if case.skipped.is_some() {
                    TestOutcome::Skipped
                } else {
                    TestOutcome::Passed
                };
                let stack_trace = failure.as_ref().and_then(|failure| failure.text.clone());
                TestCaseResult {
                    suite: suite.name.clone(),
                    class: case.class,
                    name: case.name,
                    outcome,
                    duration_ms: seconds_to_ms(&case.time),
                    failure_message: failure.and_then(|failure| failure.message),
                    source: stack_trace.as_deref().and_then(source_from_stack),
                    stack_trace,
                }
            })
        })
        .collect())
}

fn summarize(selection: TestSelection, cases: Vec<TestCaseResult>) -> TestRunResult {
    let mut summary = TestRunSummary::default();
    for case in &cases {
        match case.outcome {
            TestOutcome::Passed => summary.passed = summary.passed.saturating_add(1),
            TestOutcome::Failed => summary.failed = summary.failed.saturating_add(1),
            TestOutcome::Skipped => summary.skipped = summary.skipped.saturating_add(1),
        }
        summary.duration_ms = summary.duration_ms.saturating_add(case.duration_ms);
    }
    TestRunResult {
        selection,
        summary,
        cases,
    }
}

fn seconds_to_ms(value: &str) -> u64 {
    value.trim().parse::<f64>().ok().map_or(0, |seconds| {
        (seconds.max(0.0) * 1000.0).round().min(u64::MAX as f64) as u64
    })
}

fn source_from_stack(stack: &str) -> Option<SourceLocation> {
    for line in stack.lines() {
        let open = line.rfind('(')? + 1;
        let close = line[open..].find(')')? + open;
        let (file, line) = line[open..close].rsplit_once(':')?;
        if let Ok(line) = line.parse() {
            return Some(SourceLocation {
                file: file.into(),
                line: Some(line),
                column: None,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_junit_failures_skips_and_sources() -> Result<(), quick_xml::DeError> {
        let cases = parse_junit(
            r#"<testsuite name="Example"><testcase classname="a.Example" name="passes" time="0.1"/><testcase classname="a.Example" name="fails" time="0.2"><failure message="boom">at a.Example.fails(Example.java:42)</failure></testcase><testcase classname="a.Example" name="skip"><skipped/></testcase></testsuite>"#,
        )?;
        let result = summarize(TestSelection::default(), cases);
        assert_eq!(
            (
                result.summary.passed,
                result.summary.failed,
                result.summary.skipped
            ),
            (1, 1, 1)
        );
        assert_eq!(
            result.cases[1]
                .source
                .as_ref()
                .and_then(|source| source.line),
            Some(42)
        );
        Ok(())
    }

    #[test]
    fn parses_instrumentation_status() {
        let parsed = TestReportParser::parse_instrumentation(
            "INSTRUMENTATION_STATUS: class=a.Example\nINSTRUMENTATION_STATUS: test=passes\nINSTRUMENTATION_STATUS_CODE: 0\n",
            TestSelection::default(),
        );
        assert_eq!(parsed.result.summary.passed, 1);
    }
}
