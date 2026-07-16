use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
    time::Duration,
};

use dexdeck_protocol::{LogPriority, LogRecord};
use serde::Serialize;
use tempfile::NamedTempFile;
use tokio::task::JoinHandle;

use crate::{ByteBoundedLogBuffer, SequencedLogRecord};

pub const COPY_MAX_BYTES: usize = 100 * 1024;
const RECORDING_QUEUE_BATCHES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogExportFormat {
    Text,
    Jsonl,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogViewState {
    pub paused: bool,
    pub follow: bool,
    pub selected_sequence: Option<u64>,
}

impl LogViewState {
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn clear(&mut self, buffer: &mut ByteBoundedLogBuffer) {
        buffer.clear();
        self.selected_sequence = None;
        self.follow = true;
    }

    pub fn select_next_error(&mut self, records: &[SequencedLogRecord], reverse: bool) {
        let selected = self
            .selected_sequence
            .unwrap_or(if reverse { u64::MAX } else { 0 });
        self.selected_sequence = if reverse {
            records
                .iter()
                .rev()
                .filter(|entry| is_error(&entry.record))
                .find(|entry| entry.sequence < selected)
                .map(|entry| entry.sequence)
        } else {
            records
                .iter()
                .filter(|entry| is_error(&entry.record))
                .find(|entry| entry.sequence > selected)
                .map(|entry| entry.sequence)
        };
        self.follow = false;
    }
}

fn is_error(record: &LogRecord) -> bool {
    record.crash_boundary || record.marker.is_some() || record.priority >= LogPriority::Error
}

pub fn copy_osc52(
    output: &mut impl Write,
    text: &str,
    terminal_supports_osc52: bool,
) -> Result<(), LogIoError> {
    if !terminal_supports_osc52 {
        return Err(LogIoError::ClipboardUnsupported);
    }
    if text.len() > COPY_MAX_BYTES {
        return Err(LogIoError::CopyTooLarge {
            actual: text.len(),
            maximum: COPY_MAX_BYTES,
        });
    }
    output.write_all(b"\x1b]52;c;")?;
    write_base64(output, text.as_bytes())?;
    output.write_all(b"\x07")?;
    output.flush()?;
    Ok(())
}

pub fn export_logs(
    path: &Path,
    records: &[SequencedLogRecord],
    format: LogExportFormat,
    overwrite: bool,
) -> Result<(), LogIoError> {
    let parent = path
        .parent()
        .ok_or_else(|| LogIoError::InvalidPath(path.to_path_buf()))?;
    std::fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        for entry in records {
            write_record(&mut writer, entry.sequence, &entry.record, format)?;
        }
        writer.flush()?;
    }
    temporary.as_file().sync_all()?;
    if overwrite {
        temporary
            .persist(path)
            .map_err(|error| LogIoError::Io(error.error))?;
    } else {
        temporary.persist_noclobber(path).map_err(|error| {
            if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                LogIoError::AlreadyExists(path.to_path_buf())
            } else {
                LogIoError::Io(error.error)
            }
        })?;
    }
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordingStatus {
    pub active: bool,
    pub records_written: u64,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct LogRecorder {
    sender: Option<SyncSender<Vec<LogRecord>>>,
    status: Arc<RwLock<RecordingStatus>>,
    stopped: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl LogRecorder {
    pub fn start(
        path: &Path,
        format: LogExportFormat,
        overwrite: bool,
    ) -> Result<Self, LogIoError> {
        let parent = path
            .parent()
            .ok_or_else(|| LogIoError::InvalidPath(path.to_path_buf()))?;
        std::fs::create_dir_all(parent)?;
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(overwrite)
            .create_new(!overwrite)
            .open(path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    LogIoError::AlreadyExists(path.to_path_buf())
                } else {
                    LogIoError::Io(error)
                }
            })?;
        let (sender, receiver) = mpsc::sync_channel::<Vec<LogRecord>>(RECORDING_QUEUE_BATCHES);
        let status = Arc::new(RwLock::new(RecordingStatus {
            active: true,
            ..RecordingStatus::default()
        }));
        let stopped = Arc::new(AtomicBool::new(false));
        let writer_status = Arc::clone(&status);
        let writer_stopped = Arc::clone(&stopped);
        let task = tokio::task::spawn_blocking(move || {
            let mut writer = BufWriter::new(file);
            let mut sequence = 1_u64;
            while !writer_stopped.load(Ordering::Acquire) {
                let batch = match receiver.recv_timeout(Duration::from_millis(50)) {
                    Ok(batch) => batch,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                for record in batch {
                    if let Err(error) = write_record(&mut writer, sequence, &record, format) {
                        stop_with_error(&writer_status, &writer_stopped, error.to_string());
                        return;
                    }
                    sequence = sequence.saturating_add(1);
                    writer_status
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .records_written = sequence - 1;
                }
            }
            if let Err(error) = writer.flush().and_then(|()| writer.get_ref().sync_all()) {
                stop_with_error(&writer_status, &writer_stopped, error.to_string());
                return;
            }
            writer_status
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active = false;
        });
        Ok(Self {
            sender: Some(sender),
            status,
            stopped,
            task,
        })
    }

    pub fn try_record(&self, records: Vec<LogRecord>) -> Result<(), LogIoError> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(LogIoError::RecordingStopped(
                self.status()
                    .error
                    .unwrap_or_else(|| "recording stopped".into()),
            ));
        }
        let sender = self.sender.as_ref().ok_or_else(|| {
            LogIoError::RecordingStopped("recording has already been closed".into())
        })?;
        match sender.try_send(records) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                stop_with_error(
                    &self.status,
                    &self.stopped,
                    "recording queue overflowed; recording stopped".into(),
                );
                Err(LogIoError::RecordingOverflow)
            }
            Err(TrySendError::Disconnected(_)) => Err(LogIoError::RecordingStopped(
                self.status()
                    .error
                    .unwrap_or_else(|| "writer stopped".into()),
            )),
        }
    }

    #[must_use]
    pub fn status(&self) -> RecordingStatus {
        self.status
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub async fn stop(mut self) -> Result<RecordingStatus, LogIoError> {
        self.sender.take();
        (&mut self.task).await?;
        Ok(self.status())
    }
}

impl Drop for LogRecorder {
    fn drop(&mut self) {
        self.sender.take();
        self.stopped.store(true, Ordering::Release);
    }
}

fn stop_with_error(status: &RwLock<RecordingStatus>, stopped: &AtomicBool, error: String) {
    stopped.store(true, Ordering::Release);
    let mut status = status
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    status.active = false;
    status.error = Some(error);
}

fn write_record(
    writer: &mut impl Write,
    sequence: u64,
    record: &LogRecord,
    format: LogExportFormat,
) -> Result<(), LogIoError> {
    match format {
        LogExportFormat::Text => {
            writeln!(
                writer,
                "{} {} {} {:?} {}: {}",
                record.timestamp,
                record.process_id,
                record.thread_id,
                record.priority,
                record.tag,
                record.message
            )?;
        }
        LogExportFormat::Jsonl => {
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct ExportLine<'a> {
                schema_version: u32,
                sequence: u64,
                record: &'a LogRecord,
            }
            serde_json::to_writer(
                &mut *writer,
                &ExportLine {
                    schema_version: 1,
                    sequence,
                    record,
                },
            )?;
            writer.write_all(b"\n")?;
        }
    }
    Ok(())
}

fn write_base64(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        let encoded = [
            TABLE[usize::from(first >> 2)],
            TABLE[usize::from((first & 0x03) << 4 | second >> 4)],
            if chunk.len() > 1 {
                TABLE[usize::from((second & 0x0f) << 2 | third >> 6)]
            } else {
                b'='
            },
            if chunk.len() > 2 {
                TABLE[usize::from(third & 0x3f)]
            } else {
                b'='
            },
        ];
        writer.write_all(&encoded)?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LogIoError {
    #[error("terminal does not report OSC 52 clipboard support")]
    ClipboardUnsupported,
    #[error("copy is {actual} bytes; maximum is {maximum} bytes")]
    CopyTooLarge { actual: usize, maximum: usize },
    #[error("path has no usable parent: {0:?}")]
    InvalidPath(PathBuf),
    #[error("refusing to overwrite existing path: {0:?}")]
    AlreadyExists(PathBuf),
    #[error("recording queue overflowed; recording stopped")]
    RecordingOverflow,
    #[error("recording stopped: {0}")]
    RecordingStopped(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("recording task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::LogPriority;

    use super::*;

    fn record(message: &str) -> SequencedLogRecord {
        SequencedLogRecord {
            sequence: 7,
            record: LogRecord {
                timestamp: "2026-07-16 12:00:00.000".into(),
                process_id: 42,
                thread_id: 43,
                user_id: Some(10123),
                priority: LogPriority::Error,
                tag: "App".into(),
                message: message.into(),
                package: Some("com.example".into()),
                process: Some("com.example".into()),
                continuation: false,
                crash_boundary: false,
                group_id: Some(1),
                marker: None,
                truncated: false,
            },
        }
    }

    #[test]
    fn writes_bounded_explicit_osc52() -> Result<(), LogIoError> {
        let mut output = Vec::new();
        copy_osc52(&mut output, "hello", true)?;
        assert_eq!(output, b"\x1b]52;c;aGVsbG8=\x07");
        assert!(matches!(
            copy_osc52(&mut output, "hello", false),
            Err(LogIoError::ClipboardUnsupported)
        ));
        Ok(())
    }

    #[test]
    fn exports_atomically_and_refuses_implicit_overwrite() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("logs.jsonl");
        export_logs(&path, &[record("failed")], LogExportFormat::Jsonl, false)?;
        let line = std::fs::read_to_string(&path)?;
        assert!(line.contains("\"schemaVersion\":1"));
        assert!(matches!(
            export_logs(&path, &[], LogExportFormat::Text, false),
            Err(LogIoError::AlreadyExists(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn records_only_after_explicit_start_and_flushes_on_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("record.txt");
        assert!(!path.exists());
        let recorder = LogRecorder::start(&path, LogExportFormat::Text, false)?;
        recorder.try_record(vec![record("failed").record])?;
        let status = recorder.stop().await?;
        assert_eq!(status.records_written, 1);
        assert!(std::fs::read_to_string(path)?.contains("failed"));
        Ok(())
    }
}
