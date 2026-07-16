use std::sync::LazyLock;

use dexdeck_protocol::{LogMarkerKind, LogPriority, LogRecord};
use regex::Regex;

pub const MAX_LOGCAT_LINE_BYTES: usize = 256 * 1024;

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

#[cfg(test)]
mod tests {
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
}
