use crate::{
    catalog::{AnalysisStatus, UnknownReason},
    resolution::{self, Exclusion},
    Diagnostic, OneBasedPosition, SqlSpan,
};

/// Diagnostics and catalog coverage for one SQL input.
#[derive(Debug)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
    pub catalog: CatalogReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogReport {
    Skipped(CatalogSkipReason),
    Statements(Vec<CatalogStatementReport>),
}

impl CatalogReport {
    pub fn has_failures(&self) -> bool {
        matches!(self, Self::Statements(statements) if statements.iter().any(|s| matches!(s.status, AnalysisStatus::Failed(_))))
    }

    /// Human-readable coverage summary shared by the CLI and the language server.
    ///
    /// The first line holds the counts. Each following line starts with
    /// `{file}:{line}:{column}:` like a diagnostic, so editors and terminals can link it.
    /// Failure messages come from `AcquisitionError`'s classified text and never contain
    /// connection values or SQL.
    pub fn summary(&self, file: &str) -> String {
        let statements = match self {
            Self::Skipped(reason) => {
                let reason = match reason {
                    CatalogSkipReason::NotConfigured => "not configured",
                    CatalogSkipReason::RuleDisabled => "rule disabled",
                };
                return format!("{file}: catalog: skipped ({reason})");
            }
            Self::Statements(statements) => statements,
        };
        let (mut checked, mut not_checked, mut failed, mut deferred) = (0, 0, 0, 0);
        let mut details = Vec::new();
        // Acquisition failures usually repeat for every statement, so identical messages
        // are reported once at the first statement.
        let mut failures: Vec<(OneBasedPosition, String, usize)> = Vec::new();
        for statement in statements {
            let position = OneBasedPosition::from(statement.span.start);
            match &statement.status {
                AnalysisStatus::Complete => checked += 1,
                AnalysisStatus::Failed(error) => {
                    failed += 1;
                    let message = error.to_string();
                    match failures.iter_mut().find(|(_, m, _)| *m == message) {
                        Some((_, _, count)) => *count += 1,
                        None => failures.push((position, message, 1)),
                    }
                }
                AnalysisStatus::Excluded(reason) => {
                    not_checked += 1;
                    let reason = match statement.exclusion {
                        Some(CatalogExclusion::FileEffect) => "file effect in SQL input",
                        Some(CatalogExclusion::UnsupportedSyntax) => "unsupported syntax",
                        Some(CatalogExclusion::UnsupportedIdentifier) => "unsupported identifier",
                        Some(CatalogExclusion::TemporarySchema) => "temporary schema",
                        None => match reason {
                            UnknownReason::RecoveredSource => "recovered source",
                            UnknownReason::UnsupportedRelation => "unsupported relation",
                            UnknownReason::IncompleteCoverage => "incomplete catalog coverage",
                            UnknownReason::UnsupportedSyntax => "unsupported syntax",
                        },
                    };
                    details.push(format!(
                        "{file}:{position}: not checked against the catalog ({reason})"
                    ));
                }
            }
            if statement.recovered_source {
                deferred += 1;
                details.push(format!(
                    "{file}:{position}: table existence not checked (recovered source)"
                ));
            }
        }
        let mut summary = format!(
            "{file}: catalog: {checked} checked, {not_checked} not checked, {failed} failed"
        );
        if deferred > 0 {
            summary.push_str(&format!(", {deferred} with deferred table checks"));
        }
        for (position, message, count) in failures {
            summary.push_str(&format!(
                "\n{file}:{position}: catalog check failed: {message}"
            ));
            if count > 1 {
                summary.push_str(&format!(" (and {} more statements)", count - 1));
            }
        }
        for detail in details {
            summary.push('\n');
            summary.push_str(&detail);
        }
        summary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogSkipReason {
    NotConfigured,
    RuleDisabled,
}

/// A completed traversal can still defer a recovered source's existence check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogStatementReport {
    pub span: SqlSpan,
    pub status: AnalysisStatus,
    pub exclusion: Option<CatalogExclusion>,
    pub recovered_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogExclusion {
    FileEffect,
    UnsupportedSyntax,
    UnsupportedIdentifier,
    TemporarySchema,
}

impl From<&resolution::StatementResult> for CatalogStatementReport {
    fn from(statement: &resolution::StatementResult) -> Self {
        Self {
            span: SqlSpan::from_range(&statement.range),
            status: statement.status.clone(),
            exclusion: statement.exclusion.map(|reason| match reason {
                Exclusion::FileEffect => CatalogExclusion::FileEffect,
                Exclusion::UnsupportedSyntax => CatalogExclusion::UnsupportedSyntax,
                Exclusion::UnsupportedIdentifier => CatalogExclusion::UnsupportedIdentifier,
                Exclusion::TemporarySchema => CatalogExclusion::TemporarySchema,
            }),
            recovered_source: statement.recovered_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        catalog::{AcquisitionError, AcquisitionErrorKind, AcquisitionPhase},
        diagnostic::Position,
    };

    fn statement(line: usize, status: AnalysisStatus) -> CatalogStatementReport {
        let position = Position {
            line,
            column: 0,
            byte: 0,
        };
        CatalogStatementReport {
            span: SqlSpan {
                start: position,
                end: position,
            },
            exclusion: matches!(status, AnalysisStatus::Excluded(_))
                .then_some(CatalogExclusion::UnsupportedSyntax),
            status,
            recovered_source: false,
        }
    }

    #[test]
    fn summary_lists_each_unchecked_statement_with_its_position() {
        let report = CatalogReport::Statements(vec![
            statement(0, AnalysisStatus::Complete),
            statement(
                2,
                AnalysisStatus::Excluded(UnknownReason::UnsupportedSyntax),
            ),
        ]);
        assert_eq!(
            report.summary("q.sql"),
            "q.sql: catalog: 1 checked, 1 not checked, 0 failed\n\
             q.sql:3:1: not checked against the catalog (unsupported syntax)"
        );
    }

    #[test]
    fn summary_reports_identical_failures_once() {
        let error =
            AcquisitionError::new(AcquisitionPhase::Connect, AcquisitionErrorKind::Connection);
        let report = CatalogReport::Statements(vec![
            statement(0, AnalysisStatus::Failed(error.clone())),
            statement(1, AnalysisStatus::Failed(error.clone())),
        ]);
        assert_eq!(
            report.summary("q.sql"),
            format!(
                "q.sql: catalog: 0 checked, 0 not checked, 2 failed\n\
                 q.sql:1:1: catalog check failed: {error} (and 1 more statements)"
            )
        );
    }

    #[test]
    fn summary_reports_skip_reason() {
        assert_eq!(
            CatalogReport::Skipped(CatalogSkipReason::NotConfigured).summary("q.sql"),
            "q.sql: catalog: skipped (not configured)"
        );
    }
}
