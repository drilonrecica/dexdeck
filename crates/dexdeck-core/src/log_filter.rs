use std::{
    collections::HashSet,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
};

use dexdeck_protocol::{LogFilterSpec, LogPriority, LogRecord, LogTextSearch};
use regex::{Regex, RegexBuilder};
use tokio::task::JoinHandle;

use crate::SequencedLogRecord;

#[derive(Clone, Debug, Default)]
pub struct CompiledLogFilter {
    spec: LogFilterSpec,
    include_tags: HashSet<String>,
    exclude_tags: HashSet<String>,
    include_packages: HashSet<String>,
    exclude_packages: HashSet<String>,
    include_processes: HashSet<String>,
    exclude_processes: HashSet<String>,
    plain_search: Option<String>,
    regex_search: Option<Regex>,
}

impl CompiledLogFilter {
    pub fn compile(spec: LogFilterSpec) -> Result<Self, regex::Error> {
        let normalize = |value: &str| {
            if spec.case_sensitive {
                value.to_owned()
            } else {
                value.to_lowercase()
            }
        };
        let set = |values: &[String]| values.iter().map(|value| normalize(value)).collect();
        let (plain_search, regex_search) = match &spec.text_search {
            Some(LogTextSearch::Plain(value)) => (Some(normalize(value)), None),
            Some(LogTextSearch::Regex(value)) => (
                None,
                Some(
                    RegexBuilder::new(value)
                        .case_insensitive(!spec.case_sensitive)
                        .build()?,
                ),
            ),
            None => (None, None),
        };
        Ok(Self {
            include_tags: set(&spec.include_tags),
            exclude_tags: set(&spec.exclude_tags),
            include_packages: set(&spec.include_packages),
            exclude_packages: set(&spec.exclude_packages),
            include_processes: set(&spec.include_processes),
            exclude_processes: set(&spec.exclude_processes),
            plain_search,
            regex_search,
            spec,
        })
    }

    #[must_use]
    pub fn spec(&self) -> &LogFilterSpec {
        &self.spec
    }

    #[must_use]
    pub fn matches(&self, record: &LogRecord) -> bool {
        if self
            .spec
            .minimum_priority
            .is_some_and(|minimum| record.priority < minimum)
            || self.spec.crash_only && !record.crash_boundary && record.marker.is_none()
            || self.spec.errors
                && record.priority < LogPriority::Error
                && !record.crash_boundary
                && record.marker.is_none()
        {
            return false;
        }
        let normalize = |value: &str| {
            if self.spec.case_sensitive {
                value.to_owned()
            } else {
                value.to_lowercase()
            }
        };
        let tag = normalize(&record.tag);
        let package = record.package.as_deref().map(&normalize);
        let process = record.process.as_deref().map(&normalize);

        if self.exclude_tags.contains(&tag)
            || package
                .as_ref()
                .is_some_and(|value| self.exclude_packages.contains(value))
            || process
                .as_ref()
                .is_some_and(|value| self.exclude_processes.contains(value))
        {
            return false;
        }
        if !self.include_tags.is_empty() && !self.include_tags.contains(&tag)
            || !self.include_packages.is_empty()
                && !package.is_some_and(|value| self.include_packages.contains(&value))
            || !self.include_processes.is_empty()
                && !process.is_some_and(|value| self.include_processes.contains(&value))
        {
            return false;
        }
        let searchable = format!("{} {}", record.tag, record.message);
        if let Some(plain) = &self.plain_search
            && !normalize(&searchable).contains(plain)
        {
            return false;
        }
        if let Some(regex) = &self.regex_search
            && !regex.is_match(&searchable)
        {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogFilterSnapshot {
    pub generation: u64,
    pub sequences: Vec<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LogFilterIndex {
    requested_generation: Arc<AtomicU64>,
    snapshot: Arc<RwLock<LogFilterSnapshot>>,
}

impl LogFilterIndex {
    #[must_use]
    pub fn snapshot(&self) -> LogFilterSnapshot {
        self.snapshot
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn rebuild(&self, records: Vec<SequencedLogRecord>, spec: LogFilterSpec) -> JoinHandle<()> {
        let generation = self.requested_generation.fetch_add(1, Ordering::AcqRel) + 1;
        let requested_generation = Arc::clone(&self.requested_generation);
        let snapshot = Arc::clone(&self.snapshot);
        tokio::task::spawn_blocking(move || {
            let rebuilt = match CompiledLogFilter::compile(spec) {
                Ok(filter) => LogFilterSnapshot {
                    generation,
                    sequences: records
                        .iter()
                        .filter(|entry| filter.matches(&entry.record))
                        .map(|entry| entry.sequence)
                        .collect(),
                    error: None,
                },
                Err(error) => LogFilterSnapshot {
                    generation,
                    sequences: Vec::new(),
                    error: Some(error.to_string()),
                },
            };
            if requested_generation.load(Ordering::Acquire) == generation {
                *snapshot
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = rebuilt;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::{LogMarkerKind, LogPriority};

    use super::*;

    fn record(tag: &str, package: Option<&str>, message: &str) -> LogRecord {
        LogRecord {
            timestamp: "now".into(),
            process_id: 1,
            thread_id: 1,
            user_id: None,
            priority: LogPriority::Error,
            tag: tag.into(),
            message: message.into(),
            package: package.map(str::to_owned),
            process: Some("com.example:sync".into()),
            continuation: false,
            crash_boundary: false,
            group_id: None,
            marker: None,
            truncated: false,
        }
    }

    #[test]
    fn excludes_win_and_include_categories_are_anded() -> Result<(), regex::Error> {
        let filter = CompiledLogFilter::compile(LogFilterSpec {
            include_tags: vec!["App".into(), "Worker".into()],
            exclude_tags: vec!["Worker".into()],
            include_packages: vec!["com.example".into()],
            ..LogFilterSpec::default()
        })?;
        assert!(filter.matches(&record("App", Some("com.example"), "ok")));
        assert!(!filter.matches(&record("Worker", Some("com.example"), "no")));
        assert!(!filter.matches(&record("App", Some("other"), "no")));
        Ok(())
    }

    #[test]
    fn supports_case_aware_plain_regex_and_focus_modes() -> Result<(), regex::Error> {
        let plain = CompiledLogFilter::compile(LogFilterSpec {
            text_search: Some(LogTextSearch::Plain("FAILED".into())),
            ..LogFilterSpec::default()
        })?;
        assert!(plain.matches(&record("App", None, "request failed")));
        let regex = CompiledLogFilter::compile(LogFilterSpec {
            text_search: Some(LogTextSearch::Regex(r"request\s+failed".into())),
            case_sensitive: true,
            ..LogFilterSpec::default()
        })?;
        assert!(regex.matches(&record("App", None, "request failed")));
        let crash = CompiledLogFilter::compile(LogFilterSpec {
            crash_only: true,
            ..LogFilterSpec::default()
        })?;
        let mut crash_record = record("AndroidRuntime", None, "fatal");
        crash_record.marker = Some(LogMarkerKind::JavaCrash);
        assert!(crash.matches(&crash_record));
        assert!(!crash.matches(&record("App", None, "ordinary error")));
        Ok(())
    }
}
