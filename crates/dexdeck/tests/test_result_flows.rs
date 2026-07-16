use std::{collections::BTreeMap, fs};

use dexdeck_core::{DiagnosticNormalizer, EditorCommand};
use dexdeck_gradle::TestReportParser;
use dexdeck_protocol::{SourceLocation, TestSelection};
use dexdeck_tui::TestWorkspace;

#[test]
fn verifies_result_diagnostic_editor_and_workspace_flow() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let reports = directory.path().join("reports");
    fs::create_dir(&reports)?;
    fs::write(
        reports.join("TEST-example.xml"),
        r#"<testsuite name="Example"><testcase classname="a.Example" name="passes" time="0.01"/><testcase classname="a.Example" name="fails" time="0.02"><failure message="expected true">at a.Example.fails(Example.kt:9)</failure></testcase></testsuite>"#,
    )?;
    fs::write(reports.join("partial.xml"), "<testsuite>")?;
    let parsed = TestReportParser::parse_junit_paths(&[reports], TestSelection::default());
    assert_eq!(
        (parsed.result.summary.passed, parsed.result.summary.failed),
        (1, 1)
    );
    assert_eq!(parsed.warnings.len(), 1);

    let mut diagnostics = DiagnosticNormalizer::new();
    let normalized = diagnostics.push(
        b"e: /project/Example.kt: (9, 2): expected true\n/project/res/layout/main.xml:4:5: AAPT: error: invalid\n",
    );
    assert_eq!(normalized.len(), 2);

    let editor = directory.path().join("editor");
    fs::write(&editor, "")?;
    let command = EditorCommand::resolve(
        Some(vec![
            editor.to_string_lossy().into_owned(),
            "{path}:{line}:{column}".into(),
        ]),
        &BTreeMap::new(),
    )?;
    assert_eq!(
        command.argv(&SourceLocation {
            file: "Example.kt".into(),
            line: Some(9),
            column: Some(2),
        })?,
        ["Example.kt:9:2"]
    );

    let mut workspace = TestWorkspace::default();
    workspace.set_result(parsed.result);
    workspace.set_diagnostics(normalized);
    assert_eq!(
        workspace.selected().map(|case| case.name.as_str()),
        Some("passes")
    );
    Ok(())
}

#[test]
fn parses_passing_and_failing_instrumentation_fixtures() {
    let parsed = TestReportParser::parse_instrumentation(
        "INSTRUMENTATION_STATUS: class=a.DeviceTest\nINSTRUMENTATION_STATUS: test=passes\nINSTRUMENTATION_STATUS_CODE: 0\nINSTRUMENTATION_STATUS: class=a.DeviceTest\nINSTRUMENTATION_STATUS: test=fails\nINSTRUMENTATION_STATUS: stack=java.lang.AssertionError\nINSTRUMENTATION_STATUS_CODE: -2\n",
        TestSelection::default(),
    );
    assert_eq!(
        (parsed.result.summary.passed, parsed.result.summary.failed),
        (1, 1)
    );
}
