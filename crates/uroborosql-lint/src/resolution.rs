//! SQL-local visibility over acquired definitions; providers know nothing about aliases.
#[allow(dead_code)]
pub(crate) mod query;

use postgresql_cst_parser::tree_sitter::Range;

use self::query::{ColumnRef, Exclusion, Expr, Prepared, Select, SourceName};
use crate::catalog::{
    AbsenceKind, AcquisitionError, CatalogSnapshot, Lookup, TableDefinition, UnknownReason,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionUnknown {
    Reason(UnknownReason),
    Unavailable(AcquisitionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution<T> {
    Resolved(T),
    Absent(AbsenceKind),
    Ambiguous,
    Unknown(ResolutionUnknown),
}

impl<T> From<Lookup<T>> for Resolution<T> {
    fn from(value: Lookup<T>) -> Self {
        match value {
            Lookup::Found(value) => Self::Resolved(value),
            Lookup::Absent(reason) => Self::Absent(reason),
            Lookup::Unknown(reason) => Self::Unknown(ResolutionUnknown::Reason(reason)),
            Lookup::Unavailable(error) => Self::Unknown(ResolutionUnknown::Unavailable(error)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisStatus {
    Complete,
    Excluded(UnknownReason),
    Failed(AcquisitionError),
}

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
            let source = match select.source.request() {
                None => Lookup::Unknown(UnknownReason::RecoveredSource),
                Some(request) => match acquired {
                    Ok(snapshot) => snapshot.lookup(&request),
                    Err(error) => Lookup::Unavailable(error.clone()),
                },
            };
            let status = match &source {
                Lookup::Unknown(UnknownReason::RecoveredSource) => AnalysisStatus::Complete,
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
                        .or_else(|| output_name(&target.expr, select, &references));
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
            let source_spelling = match &select.source.name {
                SourceName::Table { schema, table } => schema.as_ref().map_or_else(
                    || table.spelling.clone(),
                    |schema| format!("{}.{}", schema.spelling, table.spelling),
                ),
                SourceName::Recovered => String::new(),
            };
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
            input: input.as_ref().clone(),
            outcome: resolve_reference(input, select, source),
        }),
        Expr::Group(operand)
        | Expr::Unary { operand }
        | Expr::IsNull { operand }
        | Expr::Cast { operand } => resolve_expr(operand, clause, select, source, results),
        Expr::Binary { left, right } => {
            resolve_expr(left, clause, select, source, results);
            resolve_expr(right, clause, select, source, results);
        }
        Expr::In { value, items } => {
            resolve_expr(value, clause, select, source, results);
            for item in items {
                resolve_expr(item, clause, select, source, results);
            }
        }
        Expr::Between {
            value,
            lower,
            upper,
        } => {
            resolve_expr(value, clause, select, source, results);
            resolve_expr(lower, clause, select, source, results);
            resolve_expr(upper, clause, select, source, results);
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
                if Some(qualifier.name.as_str()) != select.source.visible_name() {
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
                        && Some(input.column.name.as_str()) == select.source.visible_name() =>
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

fn output_name(expr: &Expr, select: &Select, references: &[Reference]) -> Option<String> {
    match expr {
        Expr::Group(operand) => output_name(operand, select, references),
        Expr::Column(_) => match &references.first()?.outcome {
            ReferenceOutcome::Lookup {
                resolution: Resolution::Resolved(ResolvedValue::Column { name, .. }),
                ..
            } => Some(name.clone()),
            ReferenceOutcome::Lookup {
                resolution: Resolution::Resolved(ResolvedValue::WholeRow(_)),
                ..
            } => select.source.visible_name().map(str::to_owned),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests;
