use std::collections::BTreeMap;
use uroborosql_lint::{
    catalog::{AnalysisStatus, UnknownReason},
    CatalogExclusion, CatalogReport, CatalogSkipReason, OneBasedPosition,
};

pub(super) fn summary(file: &str, report: &CatalogReport) -> String {
    let statements = match report {
        CatalogReport::Skipped(reason) => {
            let reason = match reason {
                CatalogSkipReason::NotConfigured => "not configured",
                CatalogSkipReason::RuleDisabled => "rule disabled",
            };
            return format!("{file}: catalog: skipped ({reason})");
        }
        CatalogReport::Statements(statements) => statements,
    };
    let (mut complete, mut excluded, mut failed, mut recovered) = (0, 0, 0, 0);
    let mut reasons: BTreeMap<String, Vec<OneBasedPosition>> = BTreeMap::new();
    for statement in statements {
        let reason = match &statement.status {
            AnalysisStatus::Complete => {
                complete += 1;
                None
            }
            AnalysisStatus::Failed(error) => {
                failed += 1;
                Some(error.to_string())
            }
            AnalysisStatus::Excluded(reason) => {
                excluded += 1;
                Some(
                    match statement.exclusion {
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
                    }
                    .to_owned(),
                )
            }
        };
        if statement.recovered_source {
            recovered += 1;
        }
        if let Some(reason) = reason {
            reasons
                .entry(reason)
                .or_default()
                .push(OneBasedPosition::from(statement.span.start));
        }
    }
    let mut summary =
        format!("{file}: catalog: complete={complete} excluded={excluded} failed={failed}");
    if recovered > 0 {
        summary.push_str(&format!("; recovered source checks deferred={recovered}"));
    }
    for (reason, positions) in reasons {
        let positions = positions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        summary.push_str(&format!("; {reason} (at {positions})"));
    }
    summary
}
