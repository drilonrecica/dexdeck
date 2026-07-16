use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fmt,
};

use thiserror::Error;

pub const REDACTED: &str = "[REDACTED]";

pub struct SensitiveValue {
    value: OsString,
}

impl SensitiveValue {
    #[must_use]
    pub fn new(value: impl Into<OsString>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn from_environment(name: &str) -> Result<Self, SecretError> {
        std::env::var_os(name).map(Self::new).ok_or_else(|| {
            SecretError::MissingEnvironmentVariable {
                name: name.to_owned(),
            }
        })
    }

    #[must_use]
    pub fn expose_os(&self) -> &OsStr {
        &self.value
    }
}

impl fmt::Debug for SensitiveValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("SensitiveValue")
            .field(&REDACTED)
            .finish()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SecretError {
    #[error("required environment variable {name} is not set")]
    MissingEnvironmentVariable { name: String },
}

#[derive(Clone, Debug, Default)]
pub struct SecretRedactor {
    patterns: Vec<String>,
}

impl SecretRedactor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, value: &SensitiveValue) {
        let Some(value) = value.expose_os().to_str() else {
            return;
        };
        if value.is_empty() || self.patterns.iter().any(|known| known == value) {
            return;
        }
        self.patterns.push(value.to_owned());
        self.patterns
            .sort_unstable_by_key(|pattern| std::cmp::Reverse(pattern.len()));
    }

    pub fn register_environment(&mut self, name: &str) -> Result<SensitiveValue, SecretError> {
        let value = SensitiveValue::from_environment(name)?;
        self.register(&value);
        Ok(value)
    }

    #[must_use]
    pub fn redact_text(&self, input: &str) -> String {
        let mut redacted = input.to_owned();
        for pattern in &self.patterns {
            redacted = redacted.replace(pattern, REDACTED);
        }
        redact_assignments(&redacted)
    }

    #[must_use]
    pub fn redact_argv(&self, argv: &[String]) -> Vec<String> {
        let mut redact_next = false;
        argv.iter()
            .map(|argument| {
                if redact_next {
                    redact_next = false;
                    return REDACTED.to_owned();
                }
                if is_sensitive_flag(argument) {
                    redact_next = true;
                    return self.redact_text(argument);
                }
                self.redact_text(argument)
            })
            .collect()
    }

    fn maximum_pattern_bytes(&self) -> usize {
        self.patterns.iter().map(String::len).max().unwrap_or(1)
    }
}

pub fn resolve_environment_references<'a>(
    redactor: &mut SecretRedactor,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeMap<String, SensitiveValue>, SecretError> {
    let mut resolved = BTreeMap::new();
    for name in names {
        let value = redactor.register_environment(name)?;
        resolved.insert(name.to_owned(), value);
    }
    Ok(resolved)
}

/// Redacts a text stream without exposing secrets split across read boundaries.
#[derive(Clone, Debug)]
pub struct StreamingSecretRedactor {
    redactor: SecretRedactor,
    pending: String,
    discarding_oversized_token: bool,
}

impl StreamingSecretRedactor {
    #[must_use]
    pub fn new(redactor: SecretRedactor) -> Self {
        Self {
            redactor,
            pending: String::new(),
            discarding_oversized_token: false,
        }
    }

    /// Returns only the prefix that is safe to publish. Call `finish` at EOF.
    pub fn push(&mut self, chunk: &str) -> String {
        const MAX_PENDING_TOKEN_BYTES: usize = 64 * 1024;
        if self.discarding_oversized_token {
            let Some((index, character)) = chunk
                .char_indices()
                .find(|(_, value)| value.is_whitespace())
            else {
                return String::new();
            };
            self.discarding_oversized_token = false;
            let remainder = &chunk[index + character.len_utf8()..];
            let mut output = character.to_string();
            output.push_str(&self.push(remainder));
            return output;
        }
        self.pending.push_str(chunk);
        let retained = self.redactor.maximum_pattern_bytes().saturating_sub(1);
        if self.pending.len() <= retained {
            return String::new();
        }
        let desired = self.pending.len() - retained;
        let Some(mut boundary) = self
            .pending
            .char_indices()
            .filter(|(index, character)| {
                character.is_whitespace() && index + character.len_utf8() <= desired
            })
            .map(|(index, character)| index + character.len_utf8())
            .next_back()
        else {
            if self.pending.len() > MAX_PENDING_TOKEN_BYTES {
                self.pending.clear();
                self.discarding_oversized_token = true;
                return REDACTED.to_owned();
            }
            return String::new();
        };
        loop {
            let previous = boundary;
            for pattern in &self.redactor.patterns {
                for (start, _) in self.pending.match_indices(pattern) {
                    let end = start + pattern.len();
                    if start < boundary && end > boundary {
                        boundary = end;
                    }
                }
            }
            if boundary == previous {
                break;
            }
        }
        let suffix = self.pending.split_off(boundary);
        let prefix = std::mem::replace(&mut self.pending, suffix);
        self.redactor.redact_text(&prefix)
    }

    #[must_use]
    pub fn finish(mut self) -> String {
        if self.discarding_oversized_token {
            return String::new();
        }
        self.redactor
            .redact_text(&std::mem::take(&mut self.pending))
    }
}

fn redact_assignments(input: &str) -> String {
    input
        .split_inclusive(char::is_whitespace)
        .map(|part| {
            let trimmed = part.trim_end_matches(char::is_whitespace);
            let whitespace = &part[trimmed.len()..];
            let redacted = trimmed
                .split_once('=')
                .filter(|(key, _)| sensitive_name(key))
                .map_or_else(
                    || trimmed.to_owned(),
                    |(key, _)| format!("{key}={REDACTED}"),
                );
            format!("{redacted}{whitespace}")
        })
        .collect()
}

fn is_sensitive_flag(value: &str) -> bool {
    value.starts_with('-') && sensitive_name(value.trim_start_matches('-'))
}

fn sensitive_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace('-', "_");
    [
        "token",
        "password",
        "passwd",
        "secret",
        "credential",
        "private_key",
        "api_key",
        "storepass",
        "keypass",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::{ErrorCategory, ErrorCode, OperationContext};
    use proptest::prelude::*;

    use super::*;
    use crate::DexError;

    #[test]
    fn sensitive_values_do_not_implement_plain_debug_output() {
        let value = SensitiveValue::new("top-secret");
        let rendered = format!("{value:?}");
        assert!(rendered.contains(REDACTED));
        assert!(!rendered.contains("top-secret"));
    }

    #[test]
    fn redacts_registered_values_and_sensitive_arguments() {
        let secret = SensitiveValue::new("s3cr3t");
        let mut redactor = SecretRedactor::new();
        redactor.register(&secret);

        assert_eq!(
            redactor.redact_text("failure for token=s3cr3t"),
            format!("failure for token={REDACTED}")
        );
        assert_eq!(
            redactor.redact_argv(&[
                "gradlew".into(),
                "--password".into(),
                "s3cr3t".into(),
                "-PapiToken=s3cr3t".into(),
            ]),
            ["gradlew", "--password", REDACTED, "-PapiToken=[REDACTED]"]
        );
    }

    #[test]
    fn streaming_redactor_hides_values_split_across_chunks() {
        let mut redactor = SecretRedactor::new();
        redactor.register(&SensitiveValue::new("boundary-secret"));
        let mut stream = StreamingSecretRedactor::new(redactor);
        let mut output = stream.push("before boundary-");
        output.push_str(&stream.push("secret after"));
        output.push_str(&stream.finish());
        assert_eq!(output, "before [REDACTED] after");
        assert!(!output.contains("boundary-secret"));
    }

    #[test]
    fn streaming_redactor_hides_sensitive_assignments_split_across_chunks() {
        let mut stream = StreamingSecretRedactor::new(SecretRedactor::new());
        let mut output = stream.push("failure pass");
        output.push_str(&stream.push("word=unregistered-value next"));
        output.push_str(&stream.finish());
        assert_eq!(output, "failure password=[REDACTED] next");
        assert!(!output.contains("unregistered-value"));
    }

    #[test]
    fn every_resolved_environment_reference_is_registered() -> Result<(), Box<dyn std::error::Error>>
    {
        let Some(path) = std::env::var_os("PATH") else {
            return Ok(());
        };
        let Some(path) = path.to_str() else {
            return Ok(());
        };
        let mut redactor = SecretRedactor::new();
        let resolved = resolve_environment_references(&mut redactor, ["PATH"])?;
        assert!(resolved.contains_key("PATH"));
        assert!(!redactor.redact_text(path).contains(path));
        Ok(())
    }

    #[test]
    fn errors_are_redacted_before_becoming_protocol_data() -> Result<(), Box<dyn std::error::Error>>
    {
        let secret = SensitiveValue::new("private-value");
        let mut redactor = SecretRedactor::new();
        redactor.register(&secret);
        let error = DexError::new(
            ErrorCode::GradleFailed,
            ErrorCategory::GradleOperation,
            "Gradle printed private-value",
            OperationContext {
                operation: "build token=private-value".into(),
                project: Some("/project".into()),
                ..OperationContext::default()
            },
            Some("remove password=private-value"),
            &redactor,
        );

        let serialized = serde_json::to_string(error.report())?;
        assert!(!serialized.contains("private-value"));
        assert!(serialized.contains(REDACTED));
        Ok(())
    }

    proptest! {
        #[test]
        fn registered_secret_never_survives_redaction(
            prefix in "[a-zA-Z0-9 ]{0,32}",
            secret in "[a-zA-Z0-9]{8,32}",
            suffix in "[a-zA-Z0-9 ]{0,32}",
        ) {
            let value = SensitiveValue::new(&secret);
            prop_assume!(secret != "REDACTED");
            let mut redactor = SecretRedactor::new();
            redactor.register(&value);
            let output = redactor.redact_text(&format!("{prefix}{secret}{suffix}"));
            prop_assert!(!output.contains(&secret));
        }
    }
}
