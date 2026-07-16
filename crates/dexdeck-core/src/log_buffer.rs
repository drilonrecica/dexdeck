use std::{collections::VecDeque, mem::size_of};

use dexdeck_protocol::LogRecord;

pub const MIN_LOG_BUFFER_BYTES: usize = 8 * 1024 * 1024;
pub const DEFAULT_LOG_BUFFER_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_LOG_BUFFER_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SequencedLogRecord {
    pub sequence: u64,
    pub record: LogRecord,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogBufferStats {
    pub capacity_bytes: usize,
    pub buffered_bytes: usize,
    pub buffered_entries: usize,
    pub dropped_entries: u64,
    pub dropped_bytes: u64,
    pub dropped_entries_since_clear: u64,
    pub dropped_bytes_since_clear: u64,
}

#[derive(Debug)]
pub struct ByteBoundedLogBuffer {
    entries: VecDeque<(SequencedLogRecord, usize)>,
    stats: LogBufferStats,
    next_sequence: u64,
}

impl Default for ByteBoundedLogBuffer {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            stats: LogBufferStats {
                capacity_bytes: DEFAULT_LOG_BUFFER_BYTES,
                ..LogBufferStats::default()
            },
            next_sequence: 1,
        }
    }
}

impl ByteBoundedLogBuffer {
    pub fn new(capacity_bytes: usize) -> Result<Self, LogBufferError> {
        if !(MIN_LOG_BUFFER_BYTES..=MAX_LOG_BUFFER_BYTES).contains(&capacity_bytes) {
            return Err(LogBufferError::InvalidCapacity {
                minimum: MIN_LOG_BUFFER_BYTES,
                maximum: MAX_LOG_BUFFER_BYTES,
                actual: capacity_bytes,
            });
        }
        Ok(Self {
            entries: VecDeque::new(),
            stats: LogBufferStats {
                capacity_bytes,
                ..LogBufferStats::default()
            },
            next_sequence: 1,
        })
    }

    pub fn from_mib(capacity_mib: u16) -> Result<Self, LogBufferError> {
        Self::new(usize::from(capacity_mib) * 1024 * 1024)
    }

    #[must_use]
    pub const fn stats(&self) -> LogBufferStats {
        self.stats
    }

    #[must_use]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &SequencedLogRecord> {
        self.entries.iter().map(|(entry, _)| entry)
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<SequencedLogRecord> {
        self.iter().cloned().collect()
    }

    pub fn push(&mut self, record: LogRecord) -> Option<u64> {
        let bytes = record_size(&record);
        if bytes > self.stats.capacity_bytes {
            self.record_drop(bytes);
            return None;
        }
        while self.stats.buffered_bytes + bytes > self.stats.capacity_bytes {
            if let Some((_, evicted_bytes)) = self.entries.pop_front() {
                self.stats.buffered_bytes -= evicted_bytes;
                self.stats.buffered_entries -= 1;
                self.record_drop(evicted_bytes);
            }
        }
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.entries
            .push_back((SequencedLogRecord { sequence, record }, bytes));
        self.stats.buffered_bytes += bytes;
        self.stats.buffered_entries += 1;
        Some(sequence)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.stats.buffered_bytes = 0;
        self.stats.buffered_entries = 0;
        self.stats.dropped_entries_since_clear = 0;
        self.stats.dropped_bytes_since_clear = 0;
    }

    fn record_drop(&mut self, bytes: usize) {
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        self.stats.dropped_entries = self.stats.dropped_entries.saturating_add(1);
        self.stats.dropped_bytes = self.stats.dropped_bytes.saturating_add(bytes);
        self.stats.dropped_entries_since_clear =
            self.stats.dropped_entries_since_clear.saturating_add(1);
        self.stats.dropped_bytes_since_clear =
            self.stats.dropped_bytes_since_clear.saturating_add(bytes);
    }
}

fn record_size(record: &LogRecord) -> usize {
    size_of::<SequencedLogRecord>()
        + record.timestamp.len()
        + record.tag.len()
        + record.message.len()
        + record.package.as_ref().map_or(0, String::len)
        + record.process.as_ref().map_or(0, String::len)
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LogBufferError {
    #[error("Logcat buffer capacity must be between {minimum} and {maximum} bytes, got {actual}")]
    InvalidCapacity {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::LogPriority;

    use super::*;

    fn record(message_bytes: usize) -> LogRecord {
        LogRecord {
            timestamp: "07-16 12:00:00.000".into(),
            process_id: 1,
            thread_id: 2,
            user_id: None,
            priority: LogPriority::Info,
            tag: "App".into(),
            message: "x".repeat(message_bytes),
            package: None,
            process: None,
            continuation: false,
            crash_boundary: false,
            group_id: Some(1),
            marker: None,
            truncated: false,
        }
    }

    #[test]
    fn rejects_invalid_capacities() {
        assert!(ByteBoundedLogBuffer::new(MIN_LOG_BUFFER_BYTES - 1).is_err());
        assert!(ByteBoundedLogBuffer::new(MAX_LOG_BUFFER_BYTES + 1).is_err());
    }

    #[test]
    fn evicts_complete_records_and_keeps_sequence_stable() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut buffer = ByteBoundedLogBuffer::new(MIN_LOG_BUFFER_BYTES)?;
        let first = buffer
            .push(record(5 * 1024 * 1024))
            .ok_or("first record was rejected")?;
        let second = buffer
            .push(record(5 * 1024 * 1024))
            .ok_or("second record was rejected")?;
        assert_eq!((first, second), (1, 2));
        assert_eq!(
            buffer
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(buffer.stats().dropped_entries, 1);
        assert!(buffer.stats().buffered_bytes <= MIN_LOG_BUFFER_BYTES);
        Ok(())
    }

    #[test]
    fn rejects_a_single_oversized_record_and_clear_resets_interval() -> Result<(), LogBufferError> {
        let mut buffer = ByteBoundedLogBuffer::new(MIN_LOG_BUFFER_BYTES)?;
        assert_eq!(buffer.push(record(MIN_LOG_BUFFER_BYTES)), None);
        assert_eq!(buffer.stats().dropped_entries_since_clear, 1);
        buffer.clear();
        assert_eq!(buffer.stats().dropped_entries, 1);
        assert_eq!(buffer.stats().dropped_entries_since_clear, 0);
        Ok(())
    }
}
