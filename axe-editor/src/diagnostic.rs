/// Diagnostic severity levels, ordered from most to least severe.
///
/// The `Ord` implementation ensures `Error > Warning > Info > Hint`,
/// which is used by `most_severe_for_line` to pick the highest priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    /// Returns a numeric priority (lower = more severe) for ordering.
    fn priority(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
            Self::Hint => 3,
        }
    }
}

impl PartialOrd for DiagnosticSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DiagnosticSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower priority number = more severe = should sort first.
        self.priority().cmp(&other.priority())
    }
}

/// A single diagnostic associated with a buffer position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferDiagnostic {
    /// 0-based line number.
    pub line: usize,
    /// 0-based start column (inclusive).
    pub col_start: usize,
    /// 0-based end column (exclusive).
    pub col_end: usize,
    /// Severity level of this diagnostic.
    pub severity: DiagnosticSeverity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source of the diagnostic (e.g., "rustc", "clippy").
    pub source: Option<String>,
    /// Diagnostic code (e.g., "E0308").
    pub code: Option<String>,
}

/// Returns an iterator over diagnostics on the given line.
pub fn diagnostics_for_line(
    diags: &[BufferDiagnostic],
    line: usize,
) -> impl Iterator<Item = &BufferDiagnostic> {
    diags.iter().filter(move |d| d.line == line)
}

/// Counts diagnostics by severity: (errors, warnings, infos, hints).
pub fn diagnostic_counts(diags: &[BufferDiagnostic]) -> (usize, usize, usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    let mut infos = 0;
    let mut hints = 0;

    for d in diags {
        match d.severity {
            DiagnosticSeverity::Error => errors += 1,
            DiagnosticSeverity::Warning => warnings += 1,
            DiagnosticSeverity::Info => infos += 1,
            DiagnosticSeverity::Hint => hints += 1,
        }
    }

    (errors, warnings, infos, hints)
}

/// Returns the most severe diagnostic severity on the given line, if any.
pub fn most_severe_for_line(diags: &[BufferDiagnostic], line: usize) -> Option<DiagnosticSeverity> {
    diagnostics_for_line(diags, line).map(|d| d.severity).min()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diag(line: usize, severity: DiagnosticSeverity) -> BufferDiagnostic {
        BufferDiagnostic {
            line,
            col_start: 0,
            col_end: 5,
            severity,
            message: format!("{severity:?} on line {line}"),
            source: None,
            code: None,
        }
    }

    #[test]
    fn severity_ordering() {
        assert!(DiagnosticSeverity::Error < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Info);
        assert!(DiagnosticSeverity::Info < DiagnosticSeverity::Hint);
    }

    #[test]
    fn diagnostics_for_line_filters() {
        let diags = vec![
            make_diag(0, DiagnosticSeverity::Error),
            make_diag(1, DiagnosticSeverity::Warning),
            make_diag(0, DiagnosticSeverity::Info),
            make_diag(2, DiagnosticSeverity::Hint),
        ];
        let line_0: Vec<_> = diagnostics_for_line(&diags, 0).collect();
        assert_eq!(line_0.len(), 2);
        assert_eq!(line_0[0].severity, DiagnosticSeverity::Error);
        assert_eq!(line_0[1].severity, DiagnosticSeverity::Info);
    }

    #[test]
    fn diagnostics_for_line_empty() {
        let diags = vec![make_diag(5, DiagnosticSeverity::Error)];
        let line_0: Vec<_> = diagnostics_for_line(&diags, 0).collect();
        assert!(line_0.is_empty());
    }

    #[test]
    fn diagnostic_counts_mixed() {
        let diags = vec![
            make_diag(0, DiagnosticSeverity::Error),
            make_diag(1, DiagnosticSeverity::Warning),
            make_diag(2, DiagnosticSeverity::Warning),
            make_diag(3, DiagnosticSeverity::Info),
            make_diag(4, DiagnosticSeverity::Hint),
            make_diag(5, DiagnosticSeverity::Hint),
            make_diag(6, DiagnosticSeverity::Hint),
        ];
        assert_eq!(diagnostic_counts(&diags), (1, 2, 1, 3));
    }

    #[test]
    fn most_severe_picks_error() {
        let diags = vec![
            make_diag(0, DiagnosticSeverity::Hint),
            make_diag(0, DiagnosticSeverity::Error),
            make_diag(0, DiagnosticSeverity::Warning),
        ];
        assert_eq!(
            most_severe_for_line(&diags, 0),
            Some(DiagnosticSeverity::Error)
        );
    }

    #[test]
    fn most_severe_empty() {
        let diags: Vec<BufferDiagnostic> = vec![];
        assert_eq!(most_severe_for_line(&diags, 0), None);
    }

    #[test]
    fn diagnostic_counts_empty() {
        let diags: Vec<BufferDiagnostic> = vec![];
        assert_eq!(diagnostic_counts(&diags), (0, 0, 0, 0));
    }
}
