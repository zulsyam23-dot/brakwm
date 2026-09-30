use serde::{Deserialize, Serialize};

use crate::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub file: String,
}

impl Diagnostic {
    pub fn new(severity: Severity, message: impl Into<String>, span: Span, file: impl Into<String>) -> Self {
        Self {
            severity,
            message: message.into(),
            span,
            file: file.into(),
        }
    }

    pub fn error(message: impl Into<String>, span: Span, file: impl Into<String>) -> Self {
        Self::new(Severity::Error, message, span, file)
    }

    pub fn warning(message: impl Into<String>, span: Span, file: impl Into<String>) -> Self {
        Self::new(Severity::Warning, message, span, file)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}:{}:{}: {}",
            self.severity,
            self.file,
            self.span.start.line,
            self.span.start.column,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DUMMY_SPAN, Span};

    #[test]
    fn formats_like_compiler() {
        let d = Diagnostic::new(Severity::Error, "oops", DUMMY_SPAN, "main.brk");
        assert!(d.to_string().contains("error"));
        assert!(d.to_string().contains("oops"));
        let _: Span = DUMMY_SPAN;
    }
}