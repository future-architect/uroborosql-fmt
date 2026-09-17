//! SQL-local visibility over acquired definitions; providers know nothing about aliases.
use postgresql_cst_parser::tree_sitter::Range;

use super::{
    input::{ColumnRef, Exclusion, Expr, Prepared, Select},
    AcquisitionError, AnalysisStatus, CatalogSnapshot, Lookup, Resolution, ResolutionUnknown,
    TableDefinition, UnknownReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceIdentity {
    pub schema: String,
    pub table: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedValue {
    Column {
        source: SourceIdentity,
        name: String,
    },
    WholeRow(SourceIdentity),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Clause {
    Select,
    Where,
}

#[derive(Debug, Clone)]
pub(crate) enum ReferenceOutcome {
    QualifierMismatch {
        range: Range,
    },
    Lookup {
        resolution: Resolution<ResolvedValue>,
        range: Range,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Reference {
    pub clause: Clause,
    pub input: ColumnRef,
    pub outcome: ReferenceOutcome,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputColumn {
    pub name: Option<String>,
    pub references: Vec<Reference>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedSelect {
    pub source: Resolution<SourceIdentity>,
    pub source_range: Range,
    pub source_spelling: String,
    pub outputs: Vec<OutputColumn>,
    pub predicate_references: Vec<Reference>,
}

#[derive(Debug, Clone)]
pub(crate) struct StatementResult {
    pub range: Range,
    pub status: AnalysisStatus,
    pub exclusion: Option<Exclusion>,
    pub resolved: Option<ResolvedSelect>,
}

pub(crate) fn resolve(
    prepared: &Prepared,
    acquired: Result<&CatalogSnapshot, &AcquisitionError>,
) -> Vec<StatementResult> {
    prepared
        .statements
        .iter()
        .map(|statement| {
            let Ok(select) = &statement.input else {
                return StatementResult {
                    range: statement.range.clone(),
                    status: AnalysisStatus::Excluded(UnknownReason::UnsupportedSyntax),
                    exclusion: statement.input.as_ref().err().copied(),
                    resolved: None,
                };
            };
            let source = match acquired {
                Ok(snapshot) => snapshot.lookup(&select.source.request()),
                Err(error) => Lookup::Unavailable(error.clone()),
            };
            let status = match &source {
                Lookup::Unknown(reason) => AnalysisStatus::Excluded(*reason),
                Lookup::Unavailable(error) => AnalysisStatus::Failed(error.clone()),
                _ => AnalysisStatus::Complete,
            };
            let source_resolution = match &source {
                Lookup::Found(table) => Resolution::Resolved(identity(table)),
                Lookup::Absent(reason) => Resolution::Absent(*reason),
                Lookup::Unknown(reason) => Resolution::Unknown(ResolutionUnknown::Reason(*reason)),
                Lookup::Unavailable(error) => {
                    Resolution::Unknown(ResolutionUnknown::Unavailable(error.clone()))
                }
            };
            let outputs = select
                .targets
                .iter()
                .map(|target| {
                    let mut references = Vec::new();
                    resolve_expr(
                        &target.expr,
                        Clause::Select,
                        select,
                        &source,
                        &mut references,
                    );
                    let name = target
                        .alias
                        .as_ref()
                        .map(|alias| alias.name.clone())
                        .or_else(|| output_name(&target.expr, select, &source));
                    OutputColumn { name, references }
                })
                .collect();
            let mut predicate_references = Vec::new();
            if let Some(predicate) = &select.predicate {
                resolve_expr(
                    predicate,
                    Clause::Where,
                    select,
                    &source,
                    &mut predicate_references,
                );
            }
            let source_spelling = select.source.schema.as_ref().map_or_else(
                || select.source.table.spelling.clone(),
                |schema| format!("{}.{}", schema.spelling, select.source.table.spelling),
            );
            StatementResult {
                range: statement.range.clone(),
                status,
                exclusion: None,
                resolved: Some(ResolvedSelect {
                    source: source_resolution,
                    source_range: select.source.range.clone(),
                    source_spelling,
                    outputs,
                    predicate_references,
                }),
            }
        })
        .collect()
}

fn identity(table: &TableDefinition) -> SourceIdentity {
    SourceIdentity {
        schema: table.schema.clone(),
        table: table.name.clone(),
    }
}

fn resolve_expr(
    expr: &Expr,
    clause: Clause,
    select: &Select,
    source: &Lookup<&TableDefinition>,
    results: &mut Vec<Reference>,
) {
    match expr {
        Expr::Column(input) => results.push(Reference {
            clause,
            input: input.clone(),
            outcome: resolve_reference(input, select, source),
        }),
        Expr::Group(operand) | Expr::Unary { operand, .. } | Expr::IsNull { operand, .. } => {
            resolve_expr(operand, clause, select, source, results)
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr(left, clause, select, source, results);
            resolve_expr(right, clause, select, source, results);
        }
        Expr::Literal => {}
    }
}

fn resolve_reference(
    input: &ColumnRef,
    select: &Select,
    source: &Lookup<&TableDefinition>,
) -> ReferenceOutcome {
    let resolution = match source {
        Lookup::Found(table) => {
            if let Some(qualifier) = &input.qualifier {
                if qualifier.name != select.source.visible_name() {
                    return ReferenceOutcome::QualifierMismatch {
                        range: qualifier.range.clone(),
                    };
                }
            }
            match table.column(&input.column.name) {
                Lookup::Found(column) => Resolution::Resolved(ResolvedValue::Column {
                    source: identity(table),
                    name: column.name.clone(),
                }),
                Lookup::Absent(_)
                    if input.qualifier.is_none()
                        && input.column.name == select.source.visible_name() =>
                {
                    Resolution::Resolved(ResolvedValue::WholeRow(identity(table)))
                }
                other => match other {
                    Lookup::Absent(reason) => Resolution::Absent(reason),
                    Lookup::Unknown(reason) => {
                        Resolution::Unknown(ResolutionUnknown::Reason(reason))
                    }
                    Lookup::Unavailable(error) => {
                        Resolution::Unknown(ResolutionUnknown::Unavailable(error))
                    }
                    Lookup::Found(_) => unreachable!(),
                },
            }
        }
        Lookup::Absent(reason) => Resolution::Absent(*reason),
        Lookup::Unknown(reason) => Resolution::Unknown(ResolutionUnknown::Reason(*reason)),
        Lookup::Unavailable(error) => {
            Resolution::Unknown(ResolutionUnknown::Unavailable(error.clone()))
        }
    };
    ReferenceOutcome::Lookup {
        resolution,
        range: input.column.range.clone(),
    }
}

fn output_name(expr: &Expr, select: &Select, source: &Lookup<&TableDefinition>) -> Option<String> {
    match expr {
        Expr::Column(input) => match resolve_reference(input, select, source) {
            ReferenceOutcome::Lookup {
                resolution: Resolution::Resolved(ResolvedValue::Column { name, .. }),
                ..
            } => Some(name),
            ReferenceOutcome::Lookup {
                resolution: Resolution::Resolved(ResolvedValue::WholeRow(_)),
                ..
            } => Some(select.source.visible_name().into()),
            _ => None,
        },
        Expr::Group(operand) => output_name(operand, select, source),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
