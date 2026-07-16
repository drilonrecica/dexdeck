use std::{
    fs::{self, OpenOptions},
    io::Write,
    process::ExitCode,
    thread,
    time::Duration,
};

use dexdeck_test_support::FakeToolScenario;

fn main() -> ExitCode {
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let scenario_path = executable.with_extension("scenario.json");
    let calls_path = executable.with_extension("calls.jsonl");
    if let Ok(mut calls) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(calls_path)
    {
        let _ = serde_json::to_writer(&mut calls, &arguments);
        let _ = writeln!(calls);
    }
    let scenario = match fs::read(&scenario_path)
        .map_err(|error| error.to_string())
        .and_then(|bytes| {
            serde_json::from_slice::<FakeToolScenario>(&bytes).map_err(|error| error.to_string())
        }) {
        Ok(scenario) => scenario,
        Err(error) => {
            eprintln!("fake tool scenario error: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(response) = scenario
        .responses
        .iter()
        .find(|response| response.arguments == arguments)
    else {
        eprintln!("unexpected fake tool arguments: {arguments:?}");
        return ExitCode::FAILURE;
    };
    if response.delay_ms > 0 {
        thread::sleep(Duration::from_millis(response.delay_ms));
    }
    print!("{}", response.stdout);
    eprint!("{}", response.stderr);
    ExitCode::from(u8::try_from(response.exit_code.clamp(0, 255)).unwrap_or(1))
}
