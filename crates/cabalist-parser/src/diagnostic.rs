//! Diagnostics emitted during parsing.

use crate::span::Span;

/// Severity level for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// A hard error — the file is structurally invalid at this point.
    Error,
    /// Something suspicious but not necessarily wrong.
    Warning,
    /// Informational note.
    Info,
}

/// A diagnostic message attached to a source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
        }
    }

    pub fn info(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Info,
            message: message.into(),
            span,
        }
    }
}
