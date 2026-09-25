use crate::{
    catalog::AbsenceKind,
    diagnostic::{Diagnostic, Severity},
    resolution::{
        Projection, Reference, ReferenceOutcome, Resolution, StatementResult, WildcardMatch,
    },
    rule::Rule,
};

/// Consumes SQL-local resolution results, never acquires catalog information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoUnknownReference;

impl Rule for NoUnknownReference {
    fn name(&self) -> &'static str {
        "no-unknown-reference"
    }
    fn default_severity(&self) -> Severity {
        Severity::Error
    }
}

impl NoUnknownReference {
    pub(crate) fn diagnose(
        &self,
        statements: &[StatementResult],
        severity: Severity,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for statement in statements {
            let Some(select) = &statement.resolved else {
                continue;
            };
            match &select.source {
                Resolution::Absent(_) => diagnostics.push(Diagnostic::new(
                    self.name(),
                    severity,
                    format!("Table `{}` does not exist.", select.source_spelling),
                    &select.source_range,
                )),
                Resolution::Resolved(_) => {
                    for projection in &select.projections {
                        match projection {
                            Projection::Output(index) => {
                                for reference in &select.outputs[*index].references {
                                    if let Some(diagnostic) =
                                        self.reference_diagnostic(reference, severity)
                                    {
                                        diagnostics.push(diagnostic);
                                    }
                                }
                            }
                            Projection::Wildcard {
                                qualifier: Some((name, WildcardMatch::Mismatched)),
                            } => {
                                diagnostics.push(Diagnostic::new(
                                    self.name(),
                                    severity,
                                    format!("Unknown qualifier `{}`.", name.spelling),
                                    &name.range,
                                ));
                            }
                            Projection::Wildcard { .. } => {}
                        }
                    }
                    for reference in &select.predicate_references {
                        if let Some(diagnostic) = self.reference_diagnostic(reference, severity) {
                            diagnostics.push(diagnostic);
                        }
                    }
                }
                Resolution::Ambiguous | Resolution::Unknown(_) => {}
            }
        }
        diagnostics
    }

    fn reference_diagnostic(
        &self,
        reference: &Reference,
        severity: Severity,
    ) -> Option<Diagnostic> {
        let (message, range) = match &reference.outcome {
            ReferenceOutcome::QualifierMismatch { range } => (
                format!(
                    "Unknown qualifier `{}`.",
                    reference.input.qualifier.as_ref()?.spelling
                ),
                range,
            ),
            ReferenceOutcome::Lookup {
                resolution: Resolution::Absent(AbsenceKind::Column),
                range,
            } => (
                format!(
                    "Column `{}` does not exist.",
                    reference.input.column.spelling
                ),
                range,
            ),
            _ => return None,
        };
        Some(Diagnostic::new(self.name(), severity, message, range))
    }
}
