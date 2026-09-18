use crate::{
    catalog::{AnalysisStatus, Resolution, ResolutionUnknown, UnknownReason},
    resolution::{self, query},
    Diagnostic, SqlSpan,
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
                query::Exclusion::FileEffect => CatalogExclusion::FileEffect,
                query::Exclusion::UnsupportedSyntax => CatalogExclusion::UnsupportedSyntax,
                query::Exclusion::UnsupportedIdentifier => CatalogExclusion::UnsupportedIdentifier,
                query::Exclusion::TemporarySchema => CatalogExclusion::TemporarySchema,
            }),
            recovered_source: statement.resolved.as_ref().is_some_and(|s| {
                matches!(
                    s.source,
                    Resolution::Unknown(ResolutionUnknown::Reason(UnknownReason::RecoveredSource))
                )
            }),
        }
    }
}
