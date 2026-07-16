use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{DEFAULT_JOB_OUTPUT_BYTES, OutputBuffer, SensitiveValue};

pub struct CommandSpec {
    program: PathBuf,
    arguments: Vec<OsString>,
    working_directory: PathBuf,
    environment: BTreeMap<OsString, SensitiveValue>,
    inherit_environment: bool,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<PathBuf>,
        working_directory: impl Into<PathBuf>,
    ) -> Result<Self, ProcessError> {
        let program = program.into();
        let working_directory = working_directory.into();
        if program.as_os_str().is_empty() {
            return Err(ProcessError::EmptyProgram);
        }
        if working_directory.as_os_str().is_empty() {
            return Err(ProcessError::EmptyWorkingDirectory);
        }
        Ok(Self {
            program,
            arguments: Vec::new(),
            working_directory,
            environment: BTreeMap::new(),
            inherit_environment: false,
        })
    }

    #[must_use]
    pub fn arg(mut self, argument: impl Into<OsString>) -> Self {
        self.arguments.push(argument.into());
        self
    }

    #[must_use]
    pub fn args<I, V>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<OsString>,
    {
        self.arguments.extend(arguments.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub fn env(mut self, name: impl Into<OsString>, value: SensitiveValue) -> Self {
        self.environment.insert(name.into(), value);
        self
    }

    /// Enable only for tools that explicitly require the caller's environment.
    #[must_use]
    pub const fn inherit_environment(mut self, inherit: bool) -> Self {
        self.inherit_environment = inherit;
        self
    }

    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    #[must_use]
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }
}

impl fmt::Debug for CommandSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandSpec")
            .field("program", &self.program)
            .field("argument_count", &self.arguments.len())
            .field("working_directory", &self.working_directory)
            .field("environment_keys", &self.environment.keys())
            .field("inherit_environment", &self.inherit_environment)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerminationReason {
    Exited,
    GracefulCancellation,
    ForcedCancellation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessResult {
    pub exit_code: Option<i32>,
    pub stdout: OutputBuffer,
    pub stderr: OutputBuffer,
    pub termination: TerminationReason,
}

#[derive(Clone, Debug)]
pub struct ProcessSupervisor {
    output_capacity: usize,
    cancellation_grace: Duration,
}

impl ProcessSupervisor {
    pub fn new(output_capacity: usize, cancellation_grace: Duration) -> Result<Self, ProcessError> {
        OutputBuffer::new(output_capacity).map_err(|_| ProcessError::InvalidOutputCapacity)?;
        if cancellation_grace.is_zero() {
            return Err(ProcessError::InvalidCancellationGrace);
        }
        Ok(Self {
            output_capacity,
            cancellation_grace,
        })
    }

    pub async fn run(
        &self,
        spec: &CommandSpec,
        cancel: CancellationToken,
        force_cancel: CancellationToken,
    ) -> Result<ProcessResult, ProcessError> {
        let mut command = self.command(spec);
        platform::configure(&mut command);
        let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
            program: spec.program.clone(),
            source,
        })?;
        let process_tree = platform::ProcessTree::attach(&child)?;

        let stdout = child
            .stdout
            .take()
            .ok_or(ProcessError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ProcessError::MissingPipe("stderr"))?;
        let stdout_task = capture(stdout, self.output_capacity);
        let stderr_task = capture(stderr, self.output_capacity);

        let (status, termination) = tokio::select! {
            status = child.wait() => (status?, TerminationReason::Exited),
            () = force_cancel.cancelled() => {
                process_tree.force()?;
                (child.wait().await?, TerminationReason::ForcedCancellation)
            }
            () = cancel.cancelled() => {
                process_tree.interrupt()?;
                tokio::select! {
                    status = child.wait() => (status?, TerminationReason::GracefulCancellation),
                    () = force_cancel.cancelled() => {
                        process_tree.force()?;
                        (child.wait().await?, TerminationReason::ForcedCancellation)
                    }
                    () = tokio::time::sleep(self.cancellation_grace) => {
                        process_tree.force()?;
                        (child.wait().await?, TerminationReason::ForcedCancellation)
                    }
                }
            }
        };

        Ok(ProcessResult {
            exit_code: exit_code(status),
            stdout: join_capture(stdout_task).await?,
            stderr: join_capture(stderr_task).await?,
            termination,
        })
    }

    fn command(&self, spec: &CommandSpec) -> Command {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(&spec.working_directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if !spec.inherit_environment {
            command.env_clear();
        }
        command.envs(
            spec.environment
                .iter()
                .map(|(name, value)| (name, value.expose_os())),
        );
        command
    }
}

impl Default for ProcessSupervisor {
    fn default() -> Self {
        Self {
            output_capacity: DEFAULT_JOB_OUTPUT_BYTES,
            cancellation_grace: Duration::from_secs(3),
        }
    }
}

fn capture<R>(mut reader: R, capacity: usize) -> JoinHandle<Result<OutputBuffer, std::io::Error>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut output = OutputBuffer::new(capacity).map_err(std::io::Error::other)?;
        let mut chunk = [0_u8; 8192];
        loop {
            let read = reader.read(&mut chunk).await?;
            if read == 0 {
                return Ok(output);
            }
            output.append(&chunk[..read]);
        }
    })
}

async fn join_capture(
    task: JoinHandle<Result<OutputBuffer, std::io::Error>>,
) -> Result<OutputBuffer, ProcessError> {
    task.await
        .map_err(ProcessError::CaptureTask)?
        .map_err(ProcessError::Io)
}

fn exit_code(status: ExitStatus) -> Option<i32> {
    status.code()
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("command program cannot be empty")]
    EmptyProgram,
    #[error("command working directory cannot be empty")]
    EmptyWorkingDirectory,
    #[error("process output capacity must be greater than zero")]
    InvalidOutputCapacity,
    #[error("process cancellation grace period must be greater than zero")]
    InvalidCancellationGrace,
    #[error("failed to spawn {program:?}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("spawned process has no {0} pipe")]
    MissingPipe(&'static str),
    #[error("process output capture task failed: {0}")]
    CaptureTask(tokio::task::JoinError),
    #[error("process supervision failed: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(unix)]
mod platform {
    use rustix::process::{Pid, Signal, kill_process_group};
    use tokio::process::{Child, Command};

    use super::ProcessError;

    #[derive(Debug)]
    pub struct ProcessTree {
        process_group: Pid,
    }

    pub fn configure(command: &mut Command) {
        command.process_group(0);
    }

    impl ProcessTree {
        pub fn attach(child: &Child) -> Result<Self, ProcessError> {
            let pid = child
                .id()
                .and_then(|raw| i32::try_from(raw).ok())
                .and_then(Pid::from_raw)
                .ok_or_else(|| {
                    ProcessError::Io(std::io::Error::other("spawned process has no process ID"))
                })?;
            Ok(Self { process_group: pid })
        }

        pub fn interrupt(&self) -> Result<(), ProcessError> {
            kill_process_group(self.process_group, Signal::INT).map_err(|error| {
                ProcessError::Io(std::io::Error::from_raw_os_error(error.raw_os_error()))
            })
        }

        pub fn force(&self) -> Result<(), ProcessError> {
            match kill_process_group(self.process_group, Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
                Err(error) => Err(ProcessError::Io(std::io::Error::from_raw_os_error(
                    error.raw_os_error(),
                ))),
            }
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            let _ = self.force();
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, mem::size_of, ptr};

    use tokio::process::{Child, Command};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent},
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject,
            },
            Threading::CREATE_NEW_PROCESS_GROUP,
        },
    };

    use super::ProcessError;

    #[derive(Debug)]
    pub struct ProcessTree {
        job: isize,
        process_group: u32,
    }

    pub fn configure(command: &mut Command) {
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    impl ProcessTree {
        pub fn attach(child: &Child) -> Result<Self, ProcessError> {
            let process_group = child.id().ok_or_else(|| {
                ProcessError::Io(std::io::Error::other("spawned process has no process ID"))
            })?;
            let process_handle = child.raw_handle().ok_or_else(|| {
                ProcessError::Io(std::io::Error::other(
                    "spawned process has no process handle",
                ))
            })?;
            let job = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
            if job.is_null() {
                return Err(ProcessError::Io(std::io::Error::last_os_error()));
            }
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let configured = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast::<c_void>(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if configured == 0 {
                let error = std::io::Error::last_os_error();
                unsafe { CloseHandle(job) };
                return Err(ProcessError::Io(error));
            }
            let assigned =
                unsafe { AssignProcessToJobObject(job, process_handle.cast::<c_void>()) };
            if assigned == 0 {
                let error = std::io::Error::last_os_error();
                unsafe { CloseHandle(job) };
                return Err(ProcessError::Io(error));
            }
            Ok(Self {
                job: job as isize,
                process_group,
            })
        }

        pub fn interrupt(&self) -> Result<(), ProcessError> {
            if unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.process_group) } == 0 {
                return Err(ProcessError::Io(std::io::Error::last_os_error()));
            }
            Ok(())
        }

        pub fn force(&self) -> Result<(), ProcessError> {
            if unsafe { TerminateJobObject(self.job as HANDLE, 1) } == 0 {
                return Err(ProcessError::Io(std::io::Error::last_os_error()));
            }
            Ok(())
        }
    }

    impl Drop for ProcessTree {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.job as HANDLE) };
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::env;

    use super::*;

    #[test]
    fn command_debug_never_exposes_arguments_or_values() -> Result<(), Box<dyn std::error::Error>> {
        let spec = CommandSpec::new("program", ".")?
            .arg("secret-argument")
            .env("API_TOKEN", SensitiveValue::new("secret-value"));
        let debug = format!("{spec:?}");
        assert!(!debug.contains("secret-argument"));
        assert!(!debug.contains("secret-value"));
        assert!(debug.contains("API_TOKEN"));
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn captures_bounded_output_with_a_clean_environment()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = env::current_dir()?;
        let spec = CommandSpec::new("/usr/bin/env", directory)?
            .env("DEXDECK_VISIBLE", SensitiveValue::new("yes"));
        let supervisor = ProcessSupervisor::new(64, Duration::from_secs(1))?;
        let result = supervisor
            .run(&spec, CancellationToken::new(), CancellationToken::new())
            .await?;

        assert_eq!(result.termination, TerminationReason::Exited);
        assert!(result.stdout.text_lossy().contains("DEXDECK_VISIBLE=yes"));
        assert!(!result.stdout.text_lossy().contains("HOME="));
        assert!(result.stdout.bytes().len() <= 64);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kills_signal_resistant_process_group_after_grace()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = env::current_dir()?;
        let spec =
            CommandSpec::new("/bin/sh", directory)?.args(["-c", "trap '' INT; sleep 30 & wait"]);
        let supervisor = ProcessSupervisor::new(64, Duration::from_millis(50))?;
        let cancel = CancellationToken::new();
        let cancellation = cancel.clone();
        let task = tokio::spawn(async move {
            supervisor
                .run(&spec, cancellation, CancellationToken::new())
                .await
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();

        let result = task.await??;
        assert_eq!(result.termination, TerminationReason::ForcedCancellation);
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dropping_run_future_kills_the_process_group() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let marker = directory.path().join("survived");
        let script = format!("sleep 1; touch {}", marker.display());
        let spec = CommandSpec::new("/bin/sh", directory.path())?.args(["-c", &script]);
        let supervisor = ProcessSupervisor::new(64, Duration::from_millis(50))?;
        let task = tokio::spawn(async move {
            supervisor
                .run(&spec, CancellationToken::new(), CancellationToken::new())
                .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        task.abort();
        let _ = task.await;
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(!marker.exists());
        Ok(())
    }
}
