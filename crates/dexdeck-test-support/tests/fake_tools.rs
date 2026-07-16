use std::process::Command;

use dexdeck_test_support::{FakeTool, FakeToolResponse, FakeToolScenario};

#[test]
fn compiled_fake_tool_is_cross_platform_and_records_direct_argv()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let executable = temp
        .path()
        .join(if cfg!(windows) { "adb.exe" } else { "adb" });
    let fake = FakeTool::install(
        std::path::Path::new(env!("CARGO_BIN_EXE_dexdeck-fake-tool")),
        &executable,
        &FakeToolScenario {
            responses: vec![FakeToolResponse {
                arguments: vec!["devices".into(), "-l".into()],
                stdout: "serial device model:Pixel\n".into(),
                ..FakeToolResponse::default()
            }],
        },
    )?;
    let output = Command::new(&fake.executable)
        .args(["devices", "-l"])
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)?,
        "serial device model:Pixel\n"
    );
    assert_eq!(fake.calls()?, [vec!["devices", "-l"]]);
    Ok(())
}
