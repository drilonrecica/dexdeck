use std::{
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
    use dexdeck_protocol::{ErrorCategory, OperationContext};
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
    fn errors_are_redacted_before_becoming_protocol_data() -> Result<(), Box<dyn std::error::Error>>
    {
        let secret = SensitiveValue::new("private-value");
        let mut redactor = SecretRedactor::new();
        redactor.register(&secret);
        let error = DexError::new(
            "build.failed",
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
