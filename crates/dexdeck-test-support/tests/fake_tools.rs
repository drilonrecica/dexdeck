use std::process::Command;

use dexdeck_test_support::{FakeTool, FakeToolChunk, FakeToolResponse, FakeToolScenario};

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

#[test]
fn emits_timed_binary_chunks_including_invalid_utf8() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let executable = temp
        .path()
        .join(if cfg!(windows) { "adb.exe" } else { "adb" });
    let fake = FakeTool::install(
        std::path::Path::new(env!("CARGO_BIN_EXE_dexdeck-fake-tool")),
        &executable,
        &FakeToolScenario {
            responses: vec![FakeToolResponse {
                arguments: vec!["logcat".into()],
                stdout_bytes: vec![b'a', 0xff],
                chunks: vec![FakeToolChunk {
                    delay_ms: 1,
                    stdout: vec![b'b', b'\n'],
                    stderr: vec![0xfe],
                }],
                ..FakeToolResponse::default()
            }],
        },
    )?;
    let output = Command::new(&fake.executable).arg("logcat").output()?;
    assert_eq!(output.stdout, vec![b'a', 0xff, b'b', b'\n']);
    assert_eq!(output.stderr, vec![0xfe]);
    Ok(())
}
