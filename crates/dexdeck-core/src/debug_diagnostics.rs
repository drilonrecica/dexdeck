use std::{collections::VecDeque, fmt::Write};

use crate::SecretRedactor;

pub const DEFAULT_DEBUG_BYTES: usize = 256 * 1024;
pub const DEFAULT_DEBUG_ENTRIES: usize = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugLevel {
    Trace,
    Info,
    Warning,
    Error,
}

impl DebugLevel {
    const fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugEntry {
    pub timestamp_ms: u64,
    pub level: DebugLevel,
    pub component: String,
    pub message: String,
}

impl DebugEntry {
    fn estimated_bytes(&self) -> usize {
        self.component
            .len()
            .saturating_add(self.message.len())
            .saturating_add(48)
    }
}

#[derive(Clone, Debug)]
pub struct DebugDiagnostics {
    entries: VecDeque<DebugEntry>,
    maximum_bytes: usize,
    maximum_entries: usize,
    retained_bytes: usize,
    discarded_entries: u64,
}

impl DebugDiagnostics {
    pub fn new(maximum_bytes: usize, maximum_entries: usize) -> Result<Self, DebugDiagnosticError> {
        if maximum_bytes == 0 {
            return Err(DebugDiagnosticError::InvalidByteCapacity);
        }
        if maximum_entries == 0 {
            return Err(DebugDiagnosticError::InvalidEntryCapacity);
        }
        Ok(Self {
            entries: VecDeque::with_capacity(maximum_entries),
            maximum_bytes,
            maximum_entries,
            retained_bytes: 0,
            discarded_entries: 0,
        })
    }

    pub fn push(
        &mut self,
        timestamp_ms: u64,
        level: DebugLevel,
        component: impl Into<String>,
        message: &str,
        redactor: &SecretRedactor,
    ) {
        let mut component = sanitize(&redactor.redact_text(&component.into()));
        truncate_utf8(&mut component, self.maximum_bytes.saturating_sub(48));
        let mut message = sanitize(&redactor.redact_text(message));
        let fixed_bytes = component.len().saturating_add(48);
        let message_limit = self.maximum_bytes.saturating_sub(fixed_bytes);
        truncate_utf8(&mut message, message_limit);
        let entry = DebugEntry {
            timestamp_ms,
            level,
            component,
            message,
        };
        let entry_bytes = entry.estimated_bytes().min(self.maximum_bytes);

        while self.entries.len() >= self.maximum_entries
            || self.retained_bytes.saturating_add(entry_bytes) > self.maximum_bytes
        {
            let Some(discarded) = self.entries.pop_front() else {
                break;
            };
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(discarded.estimated_bytes());
            self.discarded_entries = self.discarded_entries.saturating_add(1);
        }
        self.retained_bytes = self.retained_bytes.saturating_add(entry_bytes);
        self.entries.push_back(entry);
    }

    #[must_use]
    pub fn entries(&self) -> &VecDeque<DebugEntry> {
        &self.entries
    }

    #[must_use]
    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    #[must_use]
    pub const fn discarded_entries(&self) -> u64 {
        self.discarded_entries
    }

    #[must_use]
    pub fn render_text(&self) -> String {
        let mut output = String::new();
        if self.discarded_entries > 0 {
            let _ = writeln!(
                output,
                "# {} earlier diagnostic entries discarded",
                self.discarded_entries
            );
        }
        for entry in &self.entries {
            let _ = writeln!(
                output,
                "[{}] {} {}: {}",
                entry.timestamp_ms,
                entry.level.label(),
                entry.component,
                entry.message
            );
        }
        output
    }
}

impl Default for DebugDiagnostics {
    fn default() -> Self {
        Self::new(DEFAULT_DEBUG_BYTES, DEFAULT_DEBUG_ENTRIES)
            .unwrap_or_else(|_| unreachable!("default capacities are non-zero"))
    }
}

fn sanitize(value: &str) -> String {
    value.replace('\r', "\\r").replace('\n', "\\n")
}

fn truncate_utf8(value: &mut String, maximum_bytes: usize) {
    if value.len() <= maximum_bytes {
        return;
    }
    let mut boundary = maximum_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DebugDiagnosticError {
    #[error("debug diagnostic byte capacity must be greater than zero")]
    InvalidByteCapacity,
    #[error("debug diagnostic entry capacity must be greater than zero")]
    InvalidEntryCapacity,
}

#[cfg(test)]
mod tests {
    use crate::SensitiveValue;

    use super::*;

    #[test]
    fn redacts_and_sanitizes_before_retaining() {
        let mut redactor = SecretRedactor::new();
        redactor.register(&SensitiveValue::new("private-value"));
        let mut diagnostics = DebugDiagnostics::new(1024, 10).unwrap_or_default();

        diagnostics.push(
            10,
            DebugLevel::Error,
            "runner\nforged",
            "token=private-value\nsecond line",
            &redactor,
        );

        let rendered = diagnostics.render_text();
        assert!(!rendered.contains("private-value"));
        assert!(rendered.contains("token=[REDACTED]"));
        assert!(rendered.contains("runner\\nforged"));
        assert!(rendered.contains("\\nsecond line"));
    }

    #[test]
    fn evicts_old_entries_by_bytes_and_count() -> Result<(), Box<dyn std::error::Error>> {
        let redactor = SecretRedactor::new();
        let mut diagnostics = DebugDiagnostics::new(128, 2)?;
        for index in 0..4 {
            diagnostics.push(
                index,
                DebugLevel::Info,
                "core",
                "bounded message",
                &redactor,
            );
        }

        assert!(diagnostics.entries().len() <= 2);
        assert!(diagnostics.retained_bytes() <= 128);
        assert!(diagnostics.discarded_entries() >= 2);
        Ok(())
    }

    #[test]
    fn rejects_unbounded_configuration() {
        assert_eq!(
            DebugDiagnostics::new(0, 1).err(),
            Some(DebugDiagnosticError::InvalidByteCapacity)
        );
        assert_eq!(
            DebugDiagnostics::new(1, 0).err(),
            Some(DebugDiagnosticError::InvalidEntryCapacity)
        );
    }
}
