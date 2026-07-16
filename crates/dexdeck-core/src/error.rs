use std::fmt;

use dexdeck_protocol::{ErrorCategory, ErrorCode, OperationContext, OperationError};

use crate::SecretRedactor;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DexError {
    report: OperationError,
}

impl DexError {
    #[must_use]
    pub fn new(
        code: ErrorCode,
        category: ErrorCategory,
        message: impl AsRef<str>,
        context: OperationContext,
        suggested_action: Option<&str>,
        redactor: &SecretRedactor,
    ) -> Self {
        Self {
            report: OperationError {
                code,
                category,
                message: redactor.redact_text(message.as_ref()),
                context: redact_context(context, redactor),
                suggested_action: suggested_action.map(|value| redactor.redact_text(value)),
            },
        }
    }

    #[must_use]
    pub const fn report(&self) -> &OperationError {
        &self.report
    }

    #[must_use]
    pub fn into_report(self) -> OperationError {
        self.report
    }
}

impl fmt::Display for DexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report.message)
    }
}

impl std::error::Error for DexError {}

fn redact_context(mut context: OperationContext, redactor: &SecretRedactor) -> OperationContext {
    context.operation = redactor.redact_text(&context.operation);
    redact_optional(&mut context.project, redactor);
    redact_optional(&mut context.module, redactor);
    redact_optional(&mut context.variant, redactor);
    redact_optional(&mut context.device, redactor);
    redact_optional(&mut context.raw_output_reference, redactor);
    context
}

fn redact_optional(value: &mut Option<String>, redactor: &SecretRedactor) {
    if let Some(value) = value {
        *value = redactor.redact_text(value);
    }
}
