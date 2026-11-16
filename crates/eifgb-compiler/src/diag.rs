use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

/// A compiler message, tied to a dotted path in the source
/// (e.g. `objects.brass_key.in`). The web editor can map these paths
/// to form fields; the CLI prints them as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sev = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        if self.path.is_empty() {
            write!(f, "{sev}: {}", self.message)
        } else {
            write!(f, "{sev}: {}: {}", self.path, self.message)
        }
    }
}

#[derive(Debug, Default)]
pub struct Diagnostics(pub Vec<Diagnostic>);

impl Diagnostics {
    pub fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(Diagnostic {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    pub fn warn(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.0.push(Diagnostic {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        });
    }

    pub fn has_errors(&self) -> bool {
        self.0.iter().any(|d| d.severity == Severity::Error)
    }
}
