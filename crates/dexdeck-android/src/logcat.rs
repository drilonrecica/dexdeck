use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{Arc, LazyLock},
    time::Duration,
};

use dexdeck_config::LogScope;
use dexdeck_core::{CommandSpec, ProcessSupervisor, TerminationReason};
use dexdeck_protocol::{LogMarkerKind, LogPriority, LogRecord};
use regex::Regex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::AdbClient;

pub const MAX_LOGCAT_LINE_BYTES: usize = 256 * 1024;
const LOGCAT_BATCH_RECORDS: usize = 256;
const LOGCAT_BATCH_BYTES: usize = 64 * 1024;
const LOGCAT_BATCH_INTERVAL: Duration = Duration::from_millis(16);
const LOGCAT_CHANNEL_BATCHES: usize = 16;

static THREADTIME: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<timestamp>(?:\d{4}-)?\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3,6}(?:\s+(?:Z|UTC|[+-]\d{4}))?)\s+(?:(?P<uid>\d+)\s+)?(?P<pid>\d+)\s+(?P<tid>\d+)\s+(?P<priority>[VDIWEF])\s+(?P<tag>.*?)\s*:\s?(?P<message>.*)$",
    )
    .ok()
});

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParserStats {
    pub physical_lines: u64,
    pub records: u64,
    pub malformed_lines: u64,
    pub invalid_utf8_lines: u64,
    pub oversized_lines: u64,
    pub truncated_lines: u64,
    pub restart_boundaries: u64,
}

#[derive(Debug)]
pub struct LogcatParser {
    partial: Vec<u8>,
    discarding_oversized: bool,
    previous: Option<LogRecord>,
    next_group_id: u64,
    stats: ParserStats,
}

impl Default for LogcatParser {
    fn default() -> Self {
        Self {
            partial: Vec::new(),
            discarding_oversized: false,
            previous: None,
            next_group_id: 1,
            stats: ParserStats::default(),
        }
    }
}

impl LogcatParser {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn stats(&self) -> ParserStats {
        self.stats
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<LogRecord> {
        let mut records = Vec::new();
        for &byte in chunk {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.partial);
                records.extend(self.parse_physical_line(&line, self.discarding_oversized));
                self.discarding_oversized = false;
            } else if self.partial.len() < MAX_LOGCAT_LINE_BYTES {
                self.partial.push(byte);
            } else {
                self.discarding_oversized = true;
            }
        }
        records
    }

    pub fn finish(&mut self) -> Vec<LogRecord> {
        if self.partial.is_empty() && !self.discarding_oversized {
            return Vec::new();
        }
        let line = std::mem::take(&mut self.partial);
        let oversized = std::mem::take(&mut self.discarding_oversized);
        self.parse_physical_line(&line, oversized)
    }

    pub fn restart(&mut self) -> Vec<LogRecord> {
        let records = self.finish();
        self.previous = None;
        self.stats.restart_boundaries = self.stats.restart_boundaries.saturating_add(1);
        records
    }

    fn parse_physical_line(&mut self, bytes: &[u8], oversized: bool) -> Vec<LogRecord> {
        self.stats.physical_lines = self.stats.physical_lines.saturating_add(1);
        if oversized {
            self.stats.oversized_lines = self.stats.oversized_lines.saturating_add(1);
            self.stats.truncated_lines = self.stats.truncated_lines.saturating_add(1);
        }
        let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
        let invalid_utf8 = std::str::from_utf8(bytes).is_err();
        if invalid_utf8 {
            self.stats.invalid_utf8_lines = self.stats.invalid_utf8_lines.saturating_add(1);
        }
        let line = String::from_utf8_lossy(bytes);
        if line.is_empty() {
            return Vec::new();
        }

        let record = if let Some(captures) = THREADTIME
            .as_ref()
            .and_then(|expression| expression.captures(&line))
        {
            let priority = match &captures["priority"] {
                "V" => LogPriority::Verbose,
                "D" => LogPriority::Debug,
                "I" => LogPriority::Info,
                "W" => LogPriority::Warning,
                "E" => LogPriority::Error,
                "F" => LogPriority::Fatal,
                _ => unreachable!("priority is constrained by the expression"),
            };
            let message = captures["message"].to_owned();
            let marker = marker_for(&captures["tag"], &message);
            let group_id = self.next_group_id;
            self.next_group_id = self.next_group_id.saturating_add(1);
            LogRecord {
                timestamp: captures["timestamp"].to_owned(),
                process_id: captures["pid"].parse().unwrap_or_default(),
                thread_id: captures["tid"].parse().unwrap_or_default(),
                user_id: captures
                    .name("uid")
                    .and_then(|value| value.as_str().parse().ok()),
                priority,
                tag: captures["tag"].trim().to_owned(),
                message,
                package: None,
                process: None,
                continuation: false,
                crash_boundary: marker.is_some(),
                group_id: Some(group_id),
                marker,
                truncated: oversized,
            }
        } else if let Some(previous) = &self.previous {
            let message = line.into_owned();
            let marker = marker_for(&previous.tag, &message);
            LogRecord {
                timestamp: previous.timestamp.clone(),
                process_id: previous.process_id,
                thread_id: previous.thread_id,
                user_id: previous.user_id,
                priority: previous.priority,
                tag: previous.tag.clone(),
                message,
                package: previous.package.clone(),
                process: previous.process.clone(),
                continuation: true,
                crash_boundary: marker.is_some(),
                group_id: previous.group_id,
                marker,
                truncated: oversized,
            }
        } else {
            self.stats.malformed_lines = self.stats.malformed_lines.saturating_add(1);
            return Vec::new();
        };
        self.stats.records = self.stats.records.saturating_add(1);
        self.previous = Some(record.clone());
        vec![record]
    }
}

fn marker_for(tag: &str, message: &str) -> Option<LogMarkerKind> {
    let lower_tag = tag.to_ascii_lowercase();
    let lower_message = message.to_ascii_lowercase();
    if lower_message.contains("fatal exception") || lower_message.contains("uncaught exception") {
        Some(LogMarkerKind::JavaCrash)
    } else if lower_tag == "debug"
        && (lower_message.contains("fatal signal") || lower_message.contains("backtrace:"))
        || lower_tag == "crash_dump64"
        || lower_tag == "crash_dump32"
    {
        Some(LogMarkerKind::NativeCrash)
    } else if lower_message.contains("anr in ") || lower_message.contains("am_anr") {
        Some(LogMarkerKind::Anr)
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogProcessSelector {
    Name(String),
    Pid(u32),
}

#[derive(Clone, Debug)]
pub struct LogcatRequest {
    pub device_serial: String,
    pub package: Option<String>,
    pub scope: LogScope,
    pub process: Option<LogProcessSelector>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogcatStatus {
    pub connected: bool,
    pub reconnects: u64,
    pub batches_sent: u64,
    pub batches_dropped: u64,
    pub records_sent: u64,
    pub records_dropped: u64,
    pub tracked_processes: usize,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
struct LogcatSelection {
    scope: LogScope,
    process: Option<LogProcessSelector>,
}

#[derive(Clone, Debug, Default)]
struct ProcessSnapshot {
    uid: Option<u32>,
    names: HashMap<u32, String>,
}

#[derive(Clone, Debug)]
pub struct LogcatService {
    adb: PathBuf,
    working_directory: PathBuf,
    supervisor: ProcessSupervisor,
}

#[derive(Debug)]
pub struct LogcatSession {
    records: mpsc::Receiver<Vec<LogRecord>>,
    status: watch::Receiver<LogcatStatus>,
    selection: watch::Sender<LogcatSelection>,
    cancel: CancellationToken,
    capture_task: JoinHandle<Result<(), LogcatError>>,
    tracker_task: Option<JoinHandle<()>>,
}

impl LogcatService {
    #[must_use]
    pub fn new(adb: PathBuf, working_directory: PathBuf, supervisor: ProcessSupervisor) -> Self {
        Self {
            adb,
            working_directory,
            supervisor,
        }
    }

    pub async fn start(&self, request: LogcatRequest) -> Result<LogcatSession, LogcatError> {
        if request.device_serial.trim().is_empty() {
            return Err(LogcatError::InvalidDevice);
        }
        if request.scope == LogScope::Application && request.package.is_none() {
            return Err(LogcatError::MissingPackage);
        }
        let (records_tx, records_rx) = mpsc::channel(LOGCAT_CHANNEL_BATCHES);
        let (status_tx, status_rx) = watch::channel(LogcatStatus::default());
        let (selection_tx, selection_rx) = watch::channel(LogcatSelection {
            scope: request.scope,
            process: request.process,
        });
        let (process_tx, process_rx) = watch::channel(ProcessSnapshot::default());
        let cancel = CancellationToken::new();
        let tracker_task = request.package.clone().map(|package| {
            let client = Arc::new(AdbClient::new(
                self.adb.clone(),
                self.working_directory.clone(),
                self.supervisor.clone(),
            ));
            let serial = request.device_serial.clone();
            let tracker_cancel = cancel.clone();
            let tracker_status = status_tx.clone();
            tokio::spawn(async move {
                track_processes(
                    client,
                    serial,
                    package,
                    process_tx,
                    tracker_status,
                    tracker_cancel,
                )
                .await;
            })
        });
        let capture_cancel = cancel.clone();
        let service = self.clone();
        let package = request.package;
        let serial = request.device_serial;
        let capture_task = tokio::spawn(async move {
            service
                .capture(
                    serial,
                    package,
                    records_tx,
                    status_tx,
                    selection_rx,
                    process_rx,
                    capture_cancel,
                )
                .await
        });
        Ok(LogcatSession {
            records: records_rx,
            status: status_rx,
            selection: selection_tx,
            cancel,
            capture_task,
            tracker_task,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn capture(
        &self,
        serial: String,
        package: Option<String>,
        records_tx: mpsc::Sender<Vec<LogRecord>>,
        status_tx: watch::Sender<LogcatStatus>,
        selection_rx: watch::Receiver<LogcatSelection>,
        process_rx: watch::Receiver<ProcessSnapshot>,
        cancel: CancellationToken,
    ) -> Result<(), LogcatError> {
        let mut parser = LogcatParser::new();
        let mut backoff = Duration::from_millis(250);
        let mut last_timestamp = None::<String>;
        let mut recent = VecDeque::<String>::new();
        let mut recent_set = HashSet::<String>::new();
        loop {
            if cancel.is_cancelled() {
                return Ok(());
            }
            let mut arguments = vec![
                "-s".to_owned(),
                serial.clone(),
                "logcat".to_owned(),
                "-v".to_owned(),
                "threadtime".to_owned(),
                "-v".to_owned(),
                "year".to_owned(),
                "-v".to_owned(),
                "usec".to_owned(),
                "-v".to_owned(),
                "UTC".to_owned(),
                "-v".to_owned(),
                "uid".to_owned(),
                "-v".to_owned(),
                "printable".to_owned(),
            ];
            if let Some(timestamp) = &last_timestamp {
                arguments.extend(["-T".to_owned(), timestamp.clone()]);
            }
            let spec = CommandSpec::new(&self.adb, &self.working_directory)?.args(arguments);
            let mut child = match self.supervisor.spawn_streaming(&spec).await {
                Ok(child) => child,
                Err(error) => {
                    status_tx.send_modify(|status| status.last_error = Some(error.to_string()));
                    if !wait_backoff(&cancel, backoff).await {
                        return Ok(());
                    }
                    backoff = (backoff * 2).min(Duration::from_secs(10));
                    continue;
                }
            };
            let mut stdout = child.take_stdout()?;
            let mut stderr = child.take_stderr()?;
            let stderr_task = tokio::spawn(async move {
                let mut sink = tokio::io::sink();
                let _ = tokio::io::copy(&mut stderr, &mut sink).await;
                let _ = sink.shutdown().await;
            });
            status_tx.send_modify(|status| {
                status.connected = true;
                status.last_error = None;
            });
            backoff = Duration::from_millis(250);
            let mut chunk = [0_u8; 8192];
            let mut batch = Vec::with_capacity(LOGCAT_BATCH_RECORDS);
            let mut batch_bytes = 0_usize;
            let mut flush = tokio::time::interval(LOGCAT_BATCH_INTERVAL);
            flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let unexpected_exit = loop {
                tokio::select! {
                    biased;
                    () = cancel.cancelled() => break false,
                    _ = flush.tick() => flush_batch(&records_tx, &status_tx, &mut batch, &mut batch_bytes),
                    read = stdout.read(&mut chunk) => {
                        let read = read?;
                        if read == 0 {
                            break true;
                        }
                        for mut record in parser.push(&chunk[..read]) {
                            enrich_record(&mut record, package.as_deref(), &process_rx.borrow());
                            if !matches_selection(&record, &selection_rx.borrow(), package.as_deref(), &process_rx.borrow()) {
                                continue;
                            }
                            last_timestamp = Some(record.timestamp.clone());
                            let key = dedup_key(&record);
                            if recent_set.contains(&key) {
                                continue;
                            }
                            recent_set.insert(key.clone());
                            recent.push_back(key);
                            if recent.len() > 4096 && let Some(expired) = recent.pop_front() {
                                recent_set.remove(&expired);
                            }
                            batch_bytes = batch_bytes.saturating_add(log_record_bytes(&record));
                            batch.push(record);
                            if batch.len() >= LOGCAT_BATCH_RECORDS || batch_bytes >= LOGCAT_BATCH_BYTES {
                                flush_batch(&records_tx, &status_tx, &mut batch, &mut batch_bytes);
                            }
                        }
                    }
                }
            };
            flush_batch(&records_tx, &status_tx, &mut batch, &mut batch_bytes);
            drop(stdout);
            let wait_cancel = if unexpected_exit {
                CancellationToken::new()
            } else {
                cancel.clone()
            };
            let (_, termination) = child.wait(wait_cancel, CancellationToken::new()).await?;
            let _ = stderr_task.await;
            status_tx.send_modify(|status| status.connected = false);
            parser.restart();
            if cancel.is_cancelled() || termination != TerminationReason::Exited {
                return Ok(());
            }
            status_tx.send_modify(|status| status.reconnects = status.reconnects.saturating_add(1));
            if !wait_backoff(&cancel, backoff).await {
                return Ok(());
            }
            backoff = (backoff * 2).min(Duration::from_secs(10));
        }
    }
}

impl LogcatSession {
    pub async fn recv(&mut self) -> Option<Vec<LogRecord>> {
        self.records.recv().await
    }

    #[must_use]
    pub fn status(&self) -> LogcatStatus {
        self.status.borrow().clone()
    }

    pub fn set_scope(&self, scope: LogScope) {
        self.selection
            .send_modify(|selection| selection.scope = scope);
    }

    pub fn set_process(&self, process: Option<LogProcessSelector>) {
        self.selection
            .send_modify(|selection| selection.process = process);
    }

    pub async fn shutdown(mut self) -> Result<(), LogcatError> {
        self.cancel.cancel();
        (&mut self.capture_task).await??;
        if let Some(task) = self.tracker_task.take() {
            let _ = task.await;
        }
        Ok(())
    }
}

impl Drop for LogcatSession {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.capture_task.abort();
        if let Some(task) = &self.tracker_task {
            task.abort();
        }
    }
}

fn flush_batch(
    records: &mpsc::Sender<Vec<LogRecord>>,
    status: &watch::Sender<LogcatStatus>,
    batch: &mut Vec<LogRecord>,
    bytes: &mut usize,
) {
    if batch.is_empty() {
        return;
    }
    let outgoing = std::mem::take(batch);
    *bytes = 0;
    let count = u64::try_from(outgoing.len()).unwrap_or(u64::MAX);
    match records.try_send(outgoing) {
        Ok(()) => status.send_modify(|value| {
            value.batches_sent = value.batches_sent.saturating_add(1);
            value.records_sent = value.records_sent.saturating_add(count);
        }),
        Err(_) => status.send_modify(|value| {
            value.batches_dropped = value.batches_dropped.saturating_add(1);
            value.records_dropped = value.records_dropped.saturating_add(count);
        }),
    }
}

async fn wait_backoff(cancel: &CancellationToken, delay: Duration) -> bool {
    tokio::select! {
        () = cancel.cancelled() => false,
        () = tokio::time::sleep(delay) => true,
    }
}

async fn track_processes(
    client: Arc<AdbClient>,
    serial: String,
    package: String,
    updates: watch::Sender<ProcessSnapshot>,
    status: watch::Sender<LogcatStatus>,
    cancel: CancellationToken,
) {
    loop {
        let uid = resolve_package_uid(&client, &serial, &package).await;
        let names = resolve_processes(&client, &serial, &package, uid).await;
        status.send_modify(|value| value.tracked_processes = names.len());
        updates.send_replace(ProcessSnapshot { uid, names });
        tokio::select! {
            () = cancel.cancelled() => return,
            () = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
}

async fn resolve_package_uid(client: &AdbClient, serial: &str, package: &str) -> Option<u32> {
    if let Some(uid) = client
        .shell(
            serial,
            &["cmd", "package", "list", "packages", "-U", package],
        )
        .await
        .ok()
        .as_deref()
        .and_then(parse_package_uid)
    {
        return Some(uid);
    }
    client
        .shell(serial, &["dumpsys", "package", package])
        .await
        .ok()
        .as_deref()
        .and_then(parse_package_uid)
}

fn parse_package_uid(output: &str) -> Option<u32> {
    output
        .split_whitespace()
        .find_map(|field| field.strip_prefix("uid:").and_then(|uid| uid.parse().ok()))
        .or_else(|| {
            output
                .lines()
                .find_map(|line| line.trim().strip_prefix("userId=")?.parse().ok())
        })
}

async fn resolve_processes(
    client: &AdbClient,
    serial: &str,
    package: &str,
    uid: Option<u32>,
) -> HashMap<u32, String> {
    let output = match client
        .shell(serial, &["ps", "-A", "-o", "UID,PID,NAME"])
        .await
    {
        Ok(output) => output,
        Err(_) => client.shell(serial, &["ps"]).await.unwrap_or_default(),
    };
    parse_processes(&output, package, uid)
}

fn parse_processes(output: &str, package: &str, uid: Option<u32>) -> HashMap<u32, String> {
    output
        .lines()
        .skip_while(|line| line.to_ascii_uppercase().contains("PID"))
        .filter_map(|line| {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let pid = fields.get(1)?.parse().ok()?;
            let name = fields.last()?.to_string();
            let numeric_uid = fields.first().and_then(|field| field.parse::<u32>().ok());
            let package_process = name == package || name.starts_with(&format!("{package}:"));
            (package_process
                && uid
                    .is_none_or(|expected| numeric_uid.is_none() || numeric_uid == Some(expected)))
            .then_some((pid, name))
        })
        .collect()
}

fn enrich_record(record: &mut LogRecord, package: Option<&str>, snapshot: &ProcessSnapshot) {
    if let Some(name) = snapshot.names.get(&record.process_id) {
        record.process = Some(name.clone());
        record.package = package.map(str::to_owned);
    }
}

fn matches_selection(
    record: &LogRecord,
    selection: &LogcatSelection,
    package: Option<&str>,
    snapshot: &ProcessSnapshot,
) -> bool {
    if selection.scope == LogScope::Application
        && !(snapshot.names.contains_key(&record.process_id)
            || package.is_some_and(|value| record.package.as_deref() == Some(value))
            || snapshot.uid.is_some() && record.user_id == snapshot.uid)
    {
        return false;
    }
    match &selection.process {
        None => true,
        Some(LogProcessSelector::Pid(pid)) => record.process_id == *pid,
        Some(LogProcessSelector::Name(name)) => record.process.as_deref() == Some(name),
    }
}

fn dedup_key(record: &LogRecord) -> String {
    format!(
        "{}\0{}\0{}\0{:?}\0{}\0{}",
        record.timestamp,
        record.process_id,
        record.thread_id,
        record.priority,
        record.tag,
        record.message
    )
}

fn log_record_bytes(record: &LogRecord) -> usize {
    record.timestamp.len() + record.tag.len() + record.message.len() + 64
}

#[derive(Debug, thiserror::Error)]
pub enum LogcatError {
    #[error("Logcat device serial cannot be empty")]
    InvalidDevice,
    #[error("application Logcat scope requires a package")]
    MissingPackage,
    #[error("Logcat capture was cancelled")]
    Cancelled,
    #[error(transparent)]
    Process(#[from] dexdeck_core::ProcessError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Logcat task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn parses_modern_and_legacy_threadtime_across_chunks() {
        let input = b"2026-07-16 12:01:02.123456 UTC 10123 42 43 E AndroidRuntime: FATAL EXCEPTION: main\n07-16 12:01:02.124 42 43 I App: ready\n";
        for split in 0..=input.len() {
            let mut parser = LogcatParser::new();
            let mut records = parser.push(&input[..split]);
            records.extend(parser.push(&input[split..]));
            assert_eq!(records.len(), 2, "split {split}");
            assert_eq!(records[0].user_id, Some(10123));
            assert_eq!(records[0].marker, Some(LogMarkerKind::JavaCrash));
            assert_eq!(records[1].user_id, None);
        }
    }

    #[test]
    fn retains_continuations_and_replaces_invalid_utf8() {
        let mut parser = LogcatParser::new();
        let records = parser.push(b"07-16 12:01:02.124 42 43 E App: failed\n  at bad\xffframe\n");
        assert_eq!(records.len(), 2);
        assert!(records[1].continuation);
        assert_eq!(records[0].group_id, records[1].group_id);
        assert_eq!(parser.stats().invalid_utf8_lines, 1);
    }

    #[test]
    fn bounds_oversized_physical_lines() {
        let mut parser = LogcatParser::new();
        let mut input = b"07-16 12:01:02.124 42 43 E App: ".to_vec();
        input.resize(MAX_LOGCAT_LINE_BYTES + 100, b'x');
        input.push(b'\n');
        let records = parser.push(&input);
        assert_eq!(records.len(), 1);
        assert!(records[0].truncated);
        assert_eq!(parser.stats().oversized_lines, 1);
    }

    #[test]
    fn restart_does_not_attach_orphaned_continuations() {
        let mut parser = LogcatParser::new();
        assert_eq!(
            parser.push(b"07-16 12:01:02.124 42 43 I App: one\n").len(),
            1
        );
        parser.restart();
        assert!(parser.push(b"orphan\n").is_empty());
        assert_eq!(parser.stats().malformed_lines, 1);
    }

    #[test]
    fn parses_modern_and_legacy_process_lists() {
        let modern = parse_processes(
            "UID PID NAME\n10123 42 com.example.app\n10123 43 com.example.app:sync\n1000 44 system_server\n",
            "com.example.app",
            Some(10123),
        );
        assert_eq!(modern.len(), 2);
        assert_eq!(
            modern.get(&43).map(String::as_str),
            Some("com.example.app:sync")
        );

        let legacy = parse_processes(
            "USER PID PPID VSIZE RSS WCHAN PC NAME\nu0_a123 51 1 0 0 0 0 com.example.app:remote\n",
            "com.example.app",
            Some(10123),
        );
        assert_eq!(
            legacy.get(&51).map(String::as_str),
            Some("com.example.app:remote")
        );
    }

    #[test]
    fn exact_pid_does_not_follow_a_restarted_process() {
        let selection = LogcatSelection {
            scope: LogScope::Application,
            process: Some(LogProcessSelector::Pid(42)),
        };
        let snapshot = ProcessSnapshot {
            uid: Some(10123),
            names: HashMap::from([(43, "com.example.app".into())]),
        };
        let mut record = LogcatParser::new()
            .push(b"07-16 12:01:02.124 10123 43 43 I App: restarted\n")
            .remove(0);
        enrich_record(&mut record, Some("com.example.app"), &snapshot);
        assert!(!matches_selection(
            &record,
            &selection,
            Some("com.example.app"),
            &snapshot
        ));
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn arbitrary_chunking_matches_whole_stream(chunk_sizes in prop::collection::vec(1_usize..64, 1..32)) {
            let input = b"07-16 12:01:02.124 42 43 I App: ready\ncontinuation\n07-16 12:01:03.125 42 44 E App: failed\n";
            let expected = LogcatParser::new().push(input);
            let mut parser = LogcatParser::new();
            let mut actual = Vec::new();
            let mut offset = 0;
            for size in chunk_sizes.into_iter().cycle() {
                if offset >= input.len() {
                    break;
                }
                let end = (offset + size).min(input.len());
                actual.extend(parser.push(&input[offset..end]));
                offset = end;
            }
            prop_assert_eq!(actual, expected);
        }

        #[test]
        fn malformed_binary_input_never_retains_an_oversized_partial(
            bytes in prop::collection::vec(any::<u8>(), 0..300_000)
        ) {
            let mut parser = LogcatParser::new();
            for chunk in bytes.chunks(137) {
                let _ = parser.push(chunk);
                prop_assert!(parser.partial.len() <= MAX_LOGCAT_LINE_BYTES);
            }
            let _ = parser.finish();
            prop_assert!(parser.partial.is_empty());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reconnects_repeated_fake_streams_and_cancels_without_orphans()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::{fs, os::unix::fs::PermissionsExt};

        let directory = tempfile::tempdir()?;
        let adb = directory.path().join("adb");
        fs::write(
            &adb,
            "#!/bin/sh\ncount=\"$0.count\"\nn=$(cat \"$count\" 2>/dev/null || echo 0)\nn=$((n + 1))\necho \"$n\" > \"$count\"\nprintf '2026-07-16 12:01:%02d.123456 UTC 42 43 I App: reconnect %d\\n' \"$n\" \"$n\"\nexit 1\n",
        )?;
        fs::set_permissions(&adb, fs::Permissions::from_mode(0o755))?;
        let service = LogcatService::new(
            adb,
            directory.path().to_path_buf(),
            ProcessSupervisor::default(),
        );
        let mut session = service
            .start(LogcatRequest {
                device_serial: "serial".into(),
                package: None,
                scope: LogScope::Device,
                process: None,
            })
            .await?;
        let first = tokio::time::timeout(Duration::from_secs(2), session.recv())
            .await?
            .ok_or("first fake Logcat stream closed")?;
        let second = tokio::time::timeout(Duration::from_secs(3), session.recv())
            .await?
            .ok_or("second fake Logcat stream closed")?;
        assert_ne!(first[0].message, second[0].message);
        assert!(session.status().reconnects >= 1);
        session.shutdown().await?;
        Ok(())
    }
}
