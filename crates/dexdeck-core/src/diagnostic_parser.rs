use std::sync::LazyLock;

use dexdeck_protocol::{Diagnostic, DiagnosticCategory, DiagnosticSeverity, SourceLocation};
use regex::Regex;

const MAX_DIAGNOSTIC_LINE_BYTES: usize = 64 * 1024;

static KOTLIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^[ewi]:\s+(.+?):\s*\((\d+),\s*(\d+)\):\s*(.+)$").ok());
static FILE_DIAGNOSTIC: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"^(.+?):(\d+)(?::(\d+))?:\s*(?:(error|warning|info):\s*)?(.+)$").ok()
});
static GRADLE_TASK: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^Execution failed for task '([^']+)'\.?").ok());

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticParserStats {
    pub lines: u64,
    pub diagnostics: u64,
    pub truncated_lines: u64,
}

#[derive(Debug, Default)]
pub struct DiagnosticNormalizer {
    partial: Vec<u8>,
    discarding: bool,
    stats: DiagnosticParserStats,
    current_task: Option<String>,
}

impl DiagnosticNormalizer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn stats(&self) -> DiagnosticParserStats {
        self.stats
    }

    pub fn push(&mut self, bytes: &[u8]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for &byte in bytes {
            if byte == b'\n' {
                let line = std::mem::take(&mut self.partial);
                diagnostics.extend(self.parse_line(&line, self.discarding));
                self.discarding = false;
            } else if self.partial.len() < MAX_DIAGNOSTIC_LINE_BYTES {
                self.partial.push(byte);
            } else {
                self.discarding = true;
            }
        }
        diagnostics
    }

    pub fn finish(&mut self) -> Vec<Diagnostic> {
        if self.partial.is_empty() && !self.discarding {
            return Vec::new();
        }
        let line = std::mem::take(&mut self.partial);
        let truncated = std::mem::take(&mut self.discarding);
        self.parse_line(&line, truncated).into_iter().collect()
    }

    fn parse_line(&mut self, bytes: &[u8], truncated: bool) -> Option<Diagnostic> {
        self.stats.lines = self.stats.lines.saturating_add(1);
        if truncated {
            self.stats.truncated_lines = self.stats.truncated_lines.saturating_add(1);
        }
        let line = String::from_utf8_lossy(bytes);
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            return None;
        }
        if let Some(task) = GRADLE_TASK
            .as_ref()
            .and_then(|expression| expression.captures(line))
            .and_then(|captures| captures.get(1))
        {
            self.current_task = Some(task.as_str().to_owned());
            return self.emit(
                DiagnosticSeverity::Error,
                DiagnosticCategory::Gradle,
                line,
                None,
                truncated,
                Some("Inspect the task output above for the root cause"),
            );
        }
        if let Some(captures) = KOTLIN
            .as_ref()
            .and_then(|expression| expression.captures(line))
        {
            let severity = match line.as_bytes().first() {
                Some(b'e') => DiagnosticSeverity::Error,
                Some(b'w') => DiagnosticSeverity::Warning,
                _ => DiagnosticSeverity::Info,
            };
            return self.emit(
                severity,
                DiagnosticCategory::Kotlin,
                captures.get(4)?.as_str(),
                location(
                    &captures[1],
                    &captures[2],
                    captures.get(3).map(|value| value.as_str()),
                ),
                truncated,
                None,
            );
        }
        if let Some(captures) = FILE_DIAGNOSTIC
            .as_ref()
            .and_then(|expression| expression.captures(line))
        {
            let file = captures.get(1)?.as_str();
            if looks_like_source(file) {
                let message = captures.get(5)?.as_str();
                let severity = severity(captures.get(4).map(|value| value.as_str()), message);
                return self.emit(
                    severity,
                    category(file, message),
                    message,
                    location(
                        file,
                        &captures[2],
                        captures.get(3).map(|value| value.as_str()),
                    ),
                    truncated,
                    None,
                );
            }
        }
        let lower = line.to_ascii_lowercase();
        let (category, suggestion) = if lower.starts_with("adb:")
            || lower.contains("failed to install")
        {
            (
                DiagnosticCategory::Adb,
                Some("Verify device state, storage, and package compatibility"),
            )
        } else if lower.contains("manifest merger failed") {
            (
                DiagnosticCategory::Manifest,
                Some("Inspect manifest merge conflicts"),
            )
        } else if lower.contains("aapt:") || lower.contains("resource linking failed") {
            (
                DiagnosticCategory::Resource,
                Some("Inspect the referenced Android resource"),
            )
        } else if lower.contains("there were failing tests") || lower.starts_with("test failed") {
            (
                DiagnosticCategory::Test,
                Some("Open the structured test failure"),
            )
        } else if lower.contains("lint found") {
            (DiagnosticCategory::Lint, Some("Inspect the lint report"))
        } else if lower.starts_with("failure:") || lower.starts_with("* what went wrong:") {
            (DiagnosticCategory::Gradle, None)
        } else {
            return None;
        };
        self.emit(
            DiagnosticSeverity::Error,
            category,
            line,
            None,
            truncated,
            suggestion,
        )
    }

    fn emit(
        &mut self,
        severity: DiagnosticSeverity,
        category: DiagnosticCategory,
        message: &str,
        location: Option<SourceLocation>,
        truncated: bool,
        suggested_action: Option<&str>,
    ) -> Option<Diagnostic> {
        self.stats.diagnostics = self.stats.diagnostics.saturating_add(1);
        Some(Diagnostic {
            severity,
            category,
            message: message.to_owned(),
            location,
            module: self.current_task.as_deref().and_then(task_module),
            variant: None,
            task: self.current_task.clone(),
            raw_context: truncated.then(|| "diagnostic line truncated at 64 KiB".into()),
            suggested_action: suggested_action.map(str::to_owned),
        })
    }
}

fn severity(value: Option<&str>, message: &str) -> DiagnosticSeverity {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("warning") => DiagnosticSeverity::Warning,
        Some("info") => DiagnosticSeverity::Info,
        Some("error") => DiagnosticSeverity::Error,
        _ if message.to_ascii_lowercase().contains("warning") => DiagnosticSeverity::Warning,
        _ => DiagnosticSeverity::Error,
    }
}

fn category(file: &str, message: &str) -> DiagnosticCategory {
    let lower_file = file.to_ascii_lowercase();
    let lower_message = message.to_ascii_lowercase();
    if lower_file.ends_with(".kt") || lower_file.ends_with(".kts") {
        DiagnosticCategory::Kotlin
    } else if lower_file.ends_with(".java") {
        DiagnosticCategory::Java
    } else if lower_file.ends_with("androidmanifest.xml") || lower_message.contains("manifest") {
        DiagnosticCategory::Manifest
    } else if lower_file.contains("/res/") || lower_message.contains("aapt") {
        DiagnosticCategory::Resource
    } else if lower_message.contains("lint") {
        DiagnosticCategory::Lint
    } else {
        DiagnosticCategory::Gradle
    }
}

fn looks_like_source(value: &str) -> bool {
    [".kt", ".kts", ".java", ".xml", ".gradle"]
        .iter()
        .any(|extension| value.to_ascii_lowercase().contains(extension))
}

fn location(file: &str, line: &str, column: Option<&str>) -> Option<SourceLocation> {
    Some(SourceLocation {
        file: file.into(),
        line: line.parse().ok(),
        column: column.and_then(|value| value.parse().ok()),
    })
}

fn task_module(task: &str) -> Option<String> {
    task.rsplit_once(':').map(|(module, _)| module.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incrementally_normalizes_compiler_resource_and_adb_failures() {
        let input = b"e: /project/App.kt: (12, 3): unresolved reference\n/project/Test.java:8: error: missing symbol\n/project/src/main/res/layout/main.xml:4:5: AAPT: error: bad resource\nadb: failed to install app.apk\n";
        for split in 0..input.len() {
            let mut parser = DiagnosticNormalizer::new();
            let mut diagnostics = parser.push(&input[..split]);
            diagnostics.extend(parser.push(&input[split..]));
            assert_eq!(diagnostics.len(), 4);
        }
    }
}
