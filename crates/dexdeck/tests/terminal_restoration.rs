#![cfg(unix)]

use std::{
    ffi::{CString, OsStr},
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, process::CommandExt},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use rustix::{
    pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
    termios::tcgetattr,
};

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    InitializationFailure,
    Panic,
}

#[test]
fn restores_real_pty_for_normal_failure_and_panic_exits() -> Result<(), Box<dyn std::error::Error>>
{
    for mode in [Mode::Normal, Mode::InitializationFailure, Mode::Panic] {
        verify(mode)?;
    }
    Ok(())
}

fn verify(mode: Mode) -> Result<(), Box<dyn std::error::Error>> {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
    grantpt(&master)?;
    unlockpt(&master)?;
    let name = ptsname(&master, Vec::new())?;
    let slave_name = CString::new(name.to_bytes())?;
    let slave = OpenOptions::new()
        .read(true)
        .write(true)
        .open(OsStr::from_bytes(name.to_bytes()))?;
    let before = tcgetattr(&slave)?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_dexdeck"));
    command
        .arg("--no-color")
        .arg("--ascii")
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave.try_clone()?));
    unsafe {
        command.pre_exec(move || {
            rustix::process::setsid().map_err(rustix_error)?;
            // A session leader acquires a controlling terminal by opening the
            // slave. This works on both Linux and macOS; TIOCSCTTY does not.
            let terminal = libc::open(slave_name.as_ptr(), libc::O_RDWR);
            if terminal == -1 {
                return Err(std::io::Error::last_os_error());
            }
            for descriptor in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
                if libc::dup2(terminal, descriptor) == -1 {
                    let error = std::io::Error::last_os_error();
                    libc::close(terminal);
                    return Err(error);
                }
            }
            if terminal > libc::STDERR_FILENO {
                libc::close(terminal);
            }
            Ok(())
        });
    }
    match mode {
        Mode::Normal => {}
        Mode::InitializationFailure => {
            command.env("DEXDECK_INTERNAL_TEST_FAIL_AFTER_RAW", "1");
        }
        Mode::Panic => {
            command.env("DEXDECK_INTERNAL_TEST_PANIC_AFTER_ENTER", "1");
        }
    }
    let mut child = command.spawn()?;
    drop(command);
    let master = File::from(master);
    let mut writer = master.try_clone()?;
    let reader = thread::spawn(move || {
        let mut master = master;
        let mut output = Vec::new();
        let _ = master.read_to_end(&mut output);
        output
    });
    if matches!(mode, Mode::Normal) {
        thread::sleep(Duration::from_millis(150));
        writer.write_all(b"q")?;
        writer.flush()?;
    }
    drop(writer);
    let status = child.wait()?;
    let after = tcgetattr(&slave)?;
    assert_eq!(after.input_modes, before.input_modes);
    assert_eq!(after.output_modes, before.output_modes);
    assert_eq!(after.control_modes, before.control_modes);
    assert_eq!(after.local_modes, before.local_modes);
    assert_eq!(after.input_speed(), before.input_speed());
    assert_eq!(after.output_speed(), before.output_speed());
    match mode {
        Mode::Normal => assert!(status.success()),
        Mode::InitializationFailure | Mode::Panic => assert!(!status.success()),
    }
    drop(slave);
    let _ = reader.join();
    Ok(())
}

fn rustix_error(error: rustix::io::Errno) -> std::io::Error {
    std::io::Error::from_raw_os_error(error.raw_os_error())
}
