//! SQL-local name resolution directly over the CST; providers know nothing about aliases.
//!
//! Every column reference is resolved against the FROM items of the SELECT that contains
//! it, then against enclosing SELECTs (correlation). A reference is reported only when
//! every scope it can see is made of catalog tables that were found; anything the
//! resolver does not model (CTEs, derived tables, set operations, LATERAL, ...) makes
//! the reference unknown instead of absent.

use postgresql_cst_parser::{
    syntax_kind::SyntaxKind as K,
    tree_sitter::{Node, Range},
};

use crate::catalog::{
    AbsenceKind, AcquisitionError, CatalogSnapshot, Lookup, TableDefinition, TableRequest,
    UnknownReason,
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

/// What a column reference refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Referent {
    /// `t.c` or `c`: a column of a catalog table.
    Column {
        schema: String,
        table: String,
        column: String,
    },
    /// `t` used as a value: the whole row of a catalog table.
    WholeRow { schema: String, table: String },
    /// `t.*`: every column of a catalog table.
    AllColumns { schema: String, table: String },
    /// An output name of the SELECT, used by ORDER BY, GROUP BY or DISTINCT ON.
    Output { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisStatus {
    Complete,
    Excluded(UnknownReason),
    Failed(AcquisitionError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exclusion {
    FileEffect,
    UnsupportedSyntax,
    UnsupportedIdentifier,
    TemporarySchema,
}

/// Catalog coverage of one top-level statement.
#[derive(Debug, Clone)]
pub(crate) struct StatementResult {
    pub range: Range,
    pub status: AnalysisStatus,
    pub exclusion: Option<Exclusion>,
    pub recovered_source: bool,
}

/// A reference that the catalog proves wrong.
#[derive(Debug, Clone)]
pub(crate) enum Finding {
    Table { spelling: String, range: Range },
    Column { spelling: String, range: Range },
    Qualifier { spelling: String, range: Range },
}

/// Acquired catalog state for one SQL input, consulted node by node.
#[derive(Debug, Clone, Copy)]
pub struct Catalog<'a> {
    acquired: Result<&'a CatalogSnapshot, &'a AcquisitionError>,
    file_effect: bool,
}

impl<'a> Catalog<'a> {
    pub fn new(
        root: &Node<'_>,
        acquired: Result<&'a CatalogSnapshot, &'a AcquisitionError>,
    ) -> Self {
        Self {
            acquired,
            file_effect: has_file_effect(root),
        }
    }

    /// Diagnoses a `relation_expr` of a FROM item whose table is certified absent.
    pub(crate) fn relation(&self, relation_expr: &Node<'_>) -> Option<Finding> {
        if self.file_effect {
            return None;
        }
        let table_ref = relation_expr
            .parent()
            .filter(|p| p.kind() == K::table_ref)?;
        let Source {
            kind: SourceKind::Table(Lookup::Absent(_)),
            ..
        } = self.source(&table_ref)
        else {
            return None;
        };
        let name = only(relation_expr, K::qualified_name).ok()?;
        let spelling = names(&name)
            .ok()?
            .iter()
            .map(|part| part.spelling.as_str())
            .collect::<Vec<_>>()
            .join(".");
        Some(Finding::Table {
            spelling,
            range: name.range(),
        })
    }

    /// Diagnoses a `columnref` that no visible scope can bind.
    pub(crate) fn reference(&self, columnref: &Node<'_>) -> Option<Finding> {
        match self.resolve(columnref) {
            Binding::Missing(finding) => Some(finding),
            _ => None,
        }
    }

    /// What a `columnref` refers to, as PostgreSQL would resolve it.
    pub fn binding(&self, columnref: &Node<'_>) -> Resolution<Referent> {
        match self.resolve(columnref) {
            Binding::Bound(referent) => Resolution::Resolved(referent),
            Binding::Ambiguous => Resolution::Ambiguous,
            Binding::Unknown(reason) => Resolution::Unknown(reason),
            Binding::Missing(Finding::Qualifier { .. }) => Resolution::Absent(AbsenceKind::Table),
            Binding::Missing(_) | Binding::NotHere => Resolution::Absent(AbsenceKind::Column),
        }
    }

    fn resolve(&self, columnref: &Node<'_>) -> Binding {
        let unsupported =
            || Binding::Unknown(ResolutionUnknown::Reason(UnknownReason::UnsupportedSyntax));
        if self.file_effect {
            return unsupported();
        }
        let Some(reference) = column_reference(columnref) else {
            return unsupported();
        };
        let mut inner = columnref.clone();
        let mut innermost = true;
        // Output names are only visible to a name that forms a whole sort or group item.
        let standalone = standalone(columnref);
        // Outside any SELECT (e.g. VALUES) nothing is known about visible names.
        if enclosing_select(&inner).is_none() {
            return unsupported();
        }
        while let Some((select, clause)) = enclosing_select(&inner) {
            // A derived table, or a parenthesized set-operation arm, sees enclosing
            // queries but not its sibling FROM items. A LATERAL one is not modelled.
            let skipped = match derived_table(&inner) {
                Some(table_ref) if lateral(&table_ref) => {
                    return Binding::Unknown(ResolutionUnknown::Reason(
                        UnknownReason::UnsupportedRelation,
                    ))
                }
                Some(_) => true,
                None => clause.kind() == K::select_with_parens,
            };
            if !skipped {
                let join = join_condition_of(&inner, &clause);
                let Some(scope) = self.scope(&select, &clause, join.as_ref()) else {
                    return unsupported();
                };
                match scope.bind(&reference, innermost && standalone) {
                    Binding::NotHere => {}
                    binding => return binding,
                }
            }
            inner = select;
            innermost = false;
        }
        Binding::Missing(match reference {
            ColumnReference {
                qualifier: Some(qualifier),
                ..
            } => Finding::Qualifier {
                spelling: qualifier.spelling,
                range: qualifier.range,
            },
            ColumnReference {
                column: Some(column),
                ..
            } => Finding::Column {
                spelling: column.spelling,
                range: column.range,
            },
            ColumnReference { column: None, .. } => return unsupported(),
        })
    }

    /// Visible FROM items for a clause of `select`, or `None` when its names are not modelled.
    /// Inside a join condition only the operands of `join` are visible.
    fn scope(
        &self,
        select: &Node<'_>,
        clause: &Node<'_>,
        join: Option<&Node<'_>>,
    ) -> Option<Scope<'a>> {
        let parts = children(select);
        let set_operation = parts
            .iter()
            .any(|part| matches!(part.kind(), K::UNION | K::INTERSECT | K::EXCEPT));
        let arm = match clause.kind() {
            K::target_list
            | K::from_clause
            | K::where_clause
            | K::group_clause
            | K::having_clause
            | K::distinct_clause => arm_of(&parts, clause)?,
            K::sort_clause if !set_operation => arm_of(&parts, clause)?,
            // A CTE body sees no FROM item of the statement that defines it.
            K::with_clause => {
                return Some(Scope {
                    sources: Vec::new(),
                    outputs: None,
                })
            }
            _ => return None,
        };
        let mut sources = Vec::new();
        if let Some(from) = join.cloned().or_else(|| {
            arm.iter()
                .find(|part| part.kind() == K::from_clause)
                .cloned()
        }) {
            for table_ref in from_items(&from) {
                sources.push(self.source(&table_ref));
            }
        }
        let outputs = matches!(
            clause.kind(),
            K::sort_clause | K::group_clause | K::distinct_clause
        )
        .then(|| {
            let mut outputs = arm
                .iter()
                .find(|part| part.kind() == K::target_list)
                .map_or_else(
                    || Outputs {
                        names: Vec::new(),
                        complete: true,
                        prefer_output: false,
                    },
                    output_names,
                );
            // GROUP BY prefers an input column; ORDER BY and DISTINCT ON an output name.
            outputs.prefer_output = clause.kind() != K::group_clause;
            outputs
        });
        Some(Scope { sources, outputs })
    }

    fn source(&self, table_ref: &Node<'_>) -> Source<'a> {
        let FromItem { visible, kind } = from_item(table_ref);
        let kind = match kind {
            FromItemKind::Table(request) => SourceKind::Table(match self.acquired {
                Ok(snapshot) => snapshot.lookup(&request),
                Err(error) => Lookup::Unavailable(error.clone()),
            }),
            FromItemKind::Recovered => {
                SourceKind::Table(Lookup::Unknown(UnknownReason::RecoveredSource))
            }
            FromItemKind::Opaque(exclusion) => SourceKind::Opaque(exclusion),
        };
        Source { visible, kind }
    }
}

/// A FROM item as written, before any catalog lookup.
struct FromItem {
    visible: Option<String>,
    kind: FromItemKind,
}

enum FromItemKind {
    Table(TableRequest),
    Recovered,
    /// See [`SourceKind::Opaque`].
    Opaque(Option<Exclusion>),
}

fn from_item(table_ref: &Node<'_>) -> FromItem {
    let opaque = |exclusion, visible| FromItem {
        visible,
        kind: FromItemKind::Opaque(exclusion),
    };
    let parts = children(table_ref);
    let (first, rest) = match parts.split_first() {
        Some((first, rest)) if rest.iter().all(|part| part.kind() == K::opt_alias_clause) => {
            (first, rest)
        }
        _ => return opaque(Some(Exclusion::UnsupportedSyntax), None),
    };
    let alias = match alias(rest.first()) {
        Ok(alias) => alias.map(|alias| alias.name),
        Err(exclusion) => return opaque(Some(exclusion), None),
    };
    match first.kind() {
        // Derived tables are understood; only their columns are not modelled.
        K::select_with_parens => opaque(None, alias),
        K::relation_expr => match relation_name(first) {
            Ok(RelationName::Recovered) => FromItem {
                visible: alias,
                kind: FromItemKind::Recovered,
            },
            Ok(RelationName::Table { schema, table }) => {
                let visible = Some(alias.unwrap_or_else(|| table.name.clone()));
                if schema.is_none() && cte_names(table_ref).contains(&table.name) {
                    return opaque(None, visible);
                }
                if schema.as_ref().is_some_and(|s| s.name == "pg_temp") {
                    return opaque(Some(Exclusion::TemporarySchema), visible);
                }
                FromItem {
                    visible,
                    kind: FromItemKind::Table(TableRequest {
                        schema: schema.map(|s| s.name),
                        name: table.name,
                    }),
                }
            }
            Err(exclusion) => opaque(Some(exclusion), alias),
        },
        _ => opaque(Some(Exclusion::UnsupportedSyntax), None),
    }
}

fn alias(clause: Option<&Node<'_>>) -> Result<Option<Identifier>, Exclusion> {
    let Some(clause) = clause else {
        return Ok(None);
    };
    let alias_clause = only(clause, K::alias_clause)?;
    match children(&alias_clause).as_slice() {
        [keyword, name] if keyword.kind() == K::AS && name.kind() == K::ColId => {
            identifier(name).map(Some)
        }
        [name] if name.kind() == K::ColId => identifier(name).map(Some),
        // Column aliases rename the relation's columns.
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}

#[derive(Debug, Clone)]
enum SourceKind<'a> {
    Table(Lookup<&'a TableDefinition>),
    /// A FROM item whose columns are not modelled. `Some` marks syntax the resolver does
    /// not understand, which leaves the statement not fully checked.
    Opaque(Option<Exclusion>),
}

#[derive(Debug, Clone)]
struct Source<'a> {
    visible: Option<String>,
    kind: SourceKind<'a>,
}

impl Source<'_> {
    fn found(&self) -> Option<&TableDefinition> {
        match &self.kind {
            SourceKind::Table(Lookup::Found(table)) => Some(table),
            _ => None,
        }
    }
}

struct Scope<'a> {
    sources: Vec<Source<'a>>,
    /// Output names visible to ORDER BY, GROUP BY and DISTINCT ON; `None` elsewhere.
    outputs: Option<Outputs>,
}

struct Outputs {
    names: Vec<String>,
    /// False when some output has a name PostgreSQL derives from an expression.
    complete: bool,
    /// Whether an output name wins over an input column of the same name.
    prefer_output: bool,
}

enum Binding {
    Bound(Referent),
    Ambiguous,
    Unknown(ResolutionUnknown),
    Missing(Finding),
    NotHere,
}

impl Source<'_> {
    /// Why the columns of a source that was not found are unknown.
    fn unknown(&self) -> ResolutionUnknown {
        match &self.kind {
            SourceKind::Table(Lookup::Unknown(reason)) => ResolutionUnknown::Reason(*reason),
            SourceKind::Table(Lookup::Unavailable(error)) => {
                ResolutionUnknown::Unavailable(error.clone())
            }
            SourceKind::Table(_) => ResolutionUnknown::Reason(UnknownReason::IncompleteCoverage),
            SourceKind::Opaque(_) => ResolutionUnknown::Reason(UnknownReason::UnsupportedRelation),
        }
    }
}

impl Scope<'_> {
    fn bind(&self, reference: &ColumnReference, output_visible: bool) -> Binding {
        let unknown = self.sources.iter().find(|source| source.found().is_none());
        if let Some(qualifier) = &reference.qualifier {
            let Some(source) = self
                .sources
                .iter()
                .find(|source| source.visible.as_deref() == Some(qualifier.name.as_str()))
            else {
                return unknown.map_or(Binding::NotHere, |source| {
                    Binding::Unknown(source.unknown())
                });
            };
            let Some(table) = source.found() else {
                return Binding::Unknown(source.unknown());
            };
            let Some(column) = &reference.column else {
                return Binding::Bound(Referent::AllColumns {
                    schema: table.schema.clone(),
                    table: table.name.clone(),
                });
            };
            return match table.column(&column.name) {
                Lookup::Found(_) => Binding::Bound(Referent::Column {
                    schema: table.schema.clone(),
                    table: table.name.clone(),
                    column: column.name.clone(),
                }),
                _ => Binding::Missing(Finding::Column {
                    spelling: column.spelling.clone(),
                    range: column.range.clone(),
                }),
            };
        }
        let Some(column) = &reference.column else {
            return Binding::Unknown(ResolutionUnknown::Reason(UnknownReason::UnsupportedSyntax));
        };
        let outputs = self.outputs.as_ref().filter(|_| output_visible);
        let output = outputs
            .filter(|outputs| outputs.names.contains(&column.name))
            .map(|_| {
                Binding::Bound(Referent::Output {
                    name: column.name.clone(),
                })
            });
        if outputs.is_some_and(|outputs| outputs.prefer_output) {
            if let Some(output) = output {
                return output;
            }
        }
        let mut tables = self
            .sources
            .iter()
            .filter_map(Source::found)
            .filter(|table| matches!(table.column(&column.name), Lookup::Found(_)));
        if let Some(table) = tables.next() {
            if tables.next().is_some() {
                return Binding::Ambiguous;
            }
            return Binding::Bound(Referent::Column {
                schema: table.schema.clone(),
                table: table.name.clone(),
                column: column.name.clone(),
            });
        }
        if let Some(source) = unknown {
            return Binding::Unknown(source.unknown());
        }
        // A real column takes precedence over a whole-row reference to the same name.
        if let Some(table) = self
            .sources
            .iter()
            .find(|source| source.visible.as_deref() == Some(column.name.as_str()))
            .and_then(Source::found)
        {
            return Binding::Bound(Referent::WholeRow {
                schema: table.schema.clone(),
                table: table.name.clone(),
            });
        }
        if let Some(output) = output {
            return output;
        }
        if outputs.is_some_and(|outputs| !outputs.complete) {
            return Binding::Unknown(ResolutionUnknown::Reason(UnknownReason::IncompleteCoverage));
        }
        Binding::NotHere
    }
}

/// Coverage report for every top-level statement.
pub(crate) fn statements(
    root: &Node<'_>,
    acquired: Result<&CatalogSnapshot, &AcquisitionError>,
) -> Vec<StatementResult> {
    let file_effect = has_file_effect(root);
    let catalog = Catalog {
        acquired,
        file_effect,
    };
    top_level_statements(root)
        .map(|statement| {
            let range = statement.range();
            if file_effect {
                return StatementResult {
                    range,
                    status: AnalysisStatus::Excluded(UnknownReason::UnsupportedSyntax),
                    exclusion: Some(Exclusion::FileEffect),
                    recovered_source: false,
                };
            }
            let sources: Vec<_> = statement
                .descendants()
                .filter(|node| node.kind() == K::table_ref && !joined(node))
                .map(|table_ref| catalog.source(&table_ref))
                .collect();
            let recovered_source = sources.iter().any(|source| {
                matches!(
                    source.kind,
                    SourceKind::Table(Lookup::Unknown(UnknownReason::RecoveredSource))
                )
            });
            // An acquisition failure outranks unsupported syntax so that it is never hidden.
            if let Some(error) = sources.iter().find_map(|source| match &source.kind {
                SourceKind::Table(Lookup::Unavailable(error)) => Some(error),
                _ => None,
            }) {
                return StatementResult {
                    range,
                    status: AnalysisStatus::Failed(error.clone()),
                    exclusion: None,
                    recovered_source,
                };
            }
            if let Some(exclusion) = sources.iter().find_map(|source| match source.kind {
                SourceKind::Opaque(exclusion) => exclusion,
                _ => None,
            }) {
                return StatementResult {
                    range,
                    status: AnalysisStatus::Excluded(UnknownReason::UnsupportedSyntax),
                    exclusion: Some(exclusion),
                    recovered_source,
                };
            }
            let mut status = AnalysisStatus::Complete;
            for source in &sources {
                match &source.kind {
                    SourceKind::Table(Lookup::Unknown(reason))
                        if *reason != UnknownReason::RecoveredSource
                            && status == AnalysisStatus::Complete =>
                    {
                        status = AnalysisStatus::Excluded(*reason);
                    }
                    _ => {}
                }
            }
            StatementResult {
                range,
                status,
                exclusion: None,
                recovered_source,
            }
        })
        .collect()
}

/// Catalog tables the input refers to; empty when a statement could change the catalog.
pub(crate) fn requests(root: &Node<'_>) -> Vec<TableRequest> {
    if has_file_effect(root) {
        return Vec::new();
    }
    let mut requests = Vec::new();
    for table_ref in root
        .descendants()
        .filter(|node| node.kind() == K::table_ref && !joined(node))
    {
        if let FromItemKind::Table(request) = from_item(&table_ref).kind {
            if !requests.contains(&request) {
                requests.push(request);
            }
        }
    }
    requests
}

fn top_level_statements<'a>(root: &Node<'a>) -> impl Iterator<Item = Node<'a>> {
    root.children()
        .into_iter()
        .filter(|n| !comment(n) && n.kind() != K::Semicolon)
}

/// Statements that can change the catalog or the search path hold checks for the whole input.
/// Function calls are assumed not to, except `set_config`, which can set `search_path`.
fn has_file_effect(root: &Node<'_>) -> bool {
    top_level_statements(root).any(|statement| {
        statement.kind() != K::SelectStmt
            || statement.descendants().any(|n| match n.kind() {
                K::into_clause | K::InsertStmt | K::UpdateStmt | K::DeleteStmt | K::MergeStmt => {
                    true
                }
                K::func_application => sets_config(&n),
                _ => false,
            })
    })
}

/// Whether `func_application` calls a function named `set_config`, in any schema.
fn sets_config(func_application: &Node<'_>) -> bool {
    let Some(name) = children(func_application)
        .into_iter()
        .find(|part| part.kind() == K::func_name)
    else {
        return false;
    };
    // The function is the last component: `f`, `s.f` or `db.s.f`.
    let function = match children(&name).as_slice() {
        [single] if single.kind() == K::type_function_name => Some(single.clone()),
        [first, indirection]
            if first.kind() == K::ColId && indirection.kind() == K::indirection =>
        {
            children(indirection).last().and_then(|element| {
                children(element)
                    .into_iter()
                    .find(|part| part.kind() == K::attr_name)
            })
        }
        _ => None,
    };
    function
        .and_then(|function| identifier(&function).ok())
        .is_some_and(|function| function.name == "set_config")
}

/// The nearest SELECT body above `node` and the clause of it that holds `node`.
fn enclosing_select<'a>(node: &Node<'a>) -> Option<(Node<'a>, Node<'a>)> {
    let mut child = node.clone();
    while let Some(parent) = child.parent() {
        if parent.kind() == K::select_no_parens {
            return Some((parent, child));
        }
        child = parent;
    }
    None
}

/// The clauses of the SELECT arm that holds `clause`. Set operations flatten their
/// arms into one body, each starting at its `SELECT` keyword.
fn arm_of<'a>(parts: &[Node<'a>], clause: &Node<'_>) -> Option<Vec<Node<'a>>> {
    let index = parts.iter().position(|part| same(part, clause))?;
    let start = parts[..=index]
        .iter()
        .rposition(|part| part.kind() == K::SELECT)?;
    let end = parts[start + 1..]
        .iter()
        .position(|part| matches!(part.kind(), K::UNION | K::INTERSECT | K::EXCEPT))
        .map_or(parts.len(), |offset| start + 1 + offset);
    // A clause after the last arm (ORDER BY) belongs to the arm only without set operations.
    Some(parts[start..end].to_vec())
}

/// The `joined_table` whose ON condition holds `node`, looking no higher than `clause`.
fn join_condition_of<'a>(node: &Node<'a>, clause: &Node<'_>) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if same(&parent, clause) {
            return None;
        }
        if parent.kind() == K::join_qual {
            return parent.parent();
        }
        current = parent.parent();
    }
    None
}

/// Table references of a FROM clause or a join, flattening joins but not derived tables.
fn from_items<'a>(from_clause: &Node<'a>) -> Vec<Node<'a>> {
    fn walk<'a>(node: &Node<'a>, items: &mut Vec<Node<'a>>) {
        for child in children(node) {
            match child.kind() {
                K::table_ref if joined(&child) => walk(&child, items),
                K::table_ref => items.push(child),
                K::from_list | K::joined_table => walk(&child, items),
                _ => {}
            }
        }
    }
    let mut items = Vec::new();
    walk(from_clause, &mut items);
    items
}

fn lateral(table_ref: &Node<'_>) -> bool {
    children(table_ref)
        .first()
        .is_some_and(|first| first.kind() == K::LATERAL_P)
}

/// The FROM item whose derived table is the SELECT body `select`.
fn derived_table<'a>(select: &Node<'a>) -> Option<Node<'a>> {
    let parens = select
        .parent()
        .filter(|parent| parent.kind() == K::select_with_parens)?;
    parens
        .parent()
        .filter(|parent| parent.kind() == K::table_ref)
}

/// Whether `table_ref` is a plain join whose FROM items stay visible by their own names.
/// A `JOIN ... USING (...) AS j` alias is not modelled, so such a join stays one opaque item.
fn joined(table_ref: &Node<'_>) -> bool {
    matches!(children(table_ref).as_slice(), [joined] if joined.kind() == K::joined_table
    && !children(joined).iter().any(|part| {
        // The alias sits in the join condition, next to `USING (...)`.
        part.kind() == K::opt_alias_clause_for_join_using
            || children(part)
                .iter()
                .any(|n| n.kind() == K::opt_alias_clause_for_join_using)
    }))
}

/// CTE names visible from `node`. A non-recursive CTE is visible to later CTEs of its
/// WITH clause and to the query, but not to its own or an earlier body.
fn cte_names(node: &Node<'_>) -> Vec<String> {
    let mut names = Vec::new();
    let mut child = node.clone();
    while let Some(ancestor) = child.parent() {
        if ancestor.kind() == K::select_no_parens {
            for with_clause in children(&ancestor)
                .into_iter()
                .filter(|part| part.kind() == K::with_clause)
            {
                let recursive = children(&with_clause)
                    .iter()
                    .any(|part| part.kind() == K::RECURSIVE);
                let inside = same(&with_clause, &child);
                for cte in with_clause.descendants().filter(|n| {
                    n.kind() == K::common_table_expr
                        && nearest_with_clause(n).is_some_and(|w| same(&w, &with_clause))
                }) {
                    if inside && !recursive && cte.range().end_byte > node.range().start_byte {
                        break;
                    }
                    if let Some(name) = children(&cte)
                        .first()
                        .filter(|n| n.kind() == K::name)
                        .and_then(|name| identifier(name).ok())
                    {
                        names.push(name.name);
                    }
                }
            }
        }
        child = ancestor;
    }
    names
}

fn nearest_with_clause<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut current = node.parent();
    while let Some(ancestor) = current {
        if ancestor.kind() == K::with_clause {
            return Some(ancestor);
        }
        current = ancestor.parent();
    }
    None
}

/// Output column names of a target list.
fn output_names(target_list: &Node<'_>) -> Outputs {
    let mut names = Vec::new();
    let mut complete = true;
    for target in children(target_list)
        .iter()
        .filter(|n| n.kind() == K::target_el)
    {
        let parts = children(target);
        match parts.as_slice() {
            [_, keyword, label] if keyword.kind() == K::AS && label.kind() == K::ColLabel => {
                // An alias that cannot be normalized may still be referenced.
                match identifier(label) {
                    Ok(name) => names.push(name.name),
                    Err(_) => complete = false,
                }
            }
            [_, label] if label.kind() == K::BareColLabel => match identifier(label) {
                Ok(name) => names.push(name.name),
                Err(_) => complete = false,
            },
            // `*` adds only input columns, which the FROM items already bind.
            [star] if star.kind() == K::Star => {}
            [expression] if expression.kind() == K::a_expr => {
                match expression
                    .descendants()
                    .find(|n| n.kind() == K::columnref)
                    .filter(|columnref| same_span(columnref, expression))
                    .and_then(|columnref| column_reference(&columnref))
                {
                    Some(ColumnReference {
                        column: Some(column),
                        ..
                    }) => names.push(column.name),
                    // `qualifier.*`
                    Some(ColumnReference { column: None, .. }) => {}
                    None => {
                        // e.g. `coalesce(...)` or `x::int`, named after the function or column.
                        complete = false;
                    }
                }
            }
            _ => complete = false,
        }
    }
    Outputs {
        names,
        complete,
        prefer_output: false,
    }
}

/// Whether `columnref` is a whole item of its clause rather than part of an expression.
/// Parentheses are dropped by PostgreSQL, and grouping lists (`ROLLUP (...)`, `CUBE (...)`,
/// `GROUP BY (a, b)`, nested at any depth) hold whole items too, as does `DISTINCT ON (...)`.
fn standalone(columnref: &Node<'_>) -> bool {
    let mut top = columnref.clone();
    let mut listed = false;
    let mut row = false;
    let mut current = columnref.parent();
    while let Some(node) = current {
        match node.kind() {
            // Only the SELECT's own ORDER BY, not one inside an aggregate or a window.
            K::sortby => return !listed && !row && sorts_select(&node),
            K::distinct_clause => return !row,
            K::group_by_item | K::rollup_clause | K::cube_clause => return true,
            K::expr_list => listed = true,
            K::implicit_row => row = true,
            _ if same_span(&node, &top) || parenthesized(&node, &top) => {}
            _ => return false,
        }
        top = node.clone();
        current = node.parent();
    }
    false
}

/// Whether `sortby` is an item of the ORDER BY of a SELECT body.
fn sorts_select(sortby: &Node<'_>) -> bool {
    sortby
        .parent()
        .filter(|list| list.kind() == K::sortby_list)
        .and_then(|list| list.parent())
        .filter(|clause| clause.kind() == K::sort_clause)
        .and_then(|clause| clause.parent())
        .is_some_and(|select| select.kind() == K::select_no_parens)
}

/// `( expr )` wrapping `inner` and nothing else.
fn parenthesized(node: &Node<'_>, inner: &Node<'_>) -> bool {
    node.kind() == K::c_expr
        && matches!(children(node).as_slice(),
            [open, expression, close] if open.kind() == K::LParen
                && close.kind() == K::RParen
                && same(expression, inner))
}

#[derive(Debug, Clone)]
struct Identifier {
    name: String,
    spelling: String,
    range: Range,
}

/// `qualifier.column`, `column`, or `qualifier.*`; `column` is `None` for a wildcard.
#[derive(Debug, Clone)]
struct ColumnReference {
    qualifier: Option<Identifier>,
    column: Option<Identifier>,
}

fn column_reference(columnref: &Node<'_>) -> Option<ColumnReference> {
    let parts = children(columnref);
    let (first, indirection) = match parts.as_slice() {
        [first] if first.kind() == K::ColId => (first, None),
        [first, indirection]
            if first.kind() == K::ColId && indirection.kind() == K::indirection =>
        {
            (first, Some(indirection))
        }
        _ => return None,
    };
    let first = identifier(first).ok()?;
    let Some(indirection) = indirection else {
        return Some(ColumnReference {
            qualifier: None,
            column: Some(first),
        });
    };
    let elements = children(indirection);
    let (head, rest) = elements.split_first()?;
    // Subscripts after the column name do not change which column is referenced.
    if rest.iter().any(|element| {
        children(element)
            .first()
            .is_none_or(|n| n.kind() != K::LBracket)
    }) {
        return None;
    }
    match children(head).as_slice() {
        [open, ..] if open.kind() == K::LBracket => Some(ColumnReference {
            qualifier: None,
            column: Some(first),
        }),
        [dot, name] if dot.kind() == K::Dot && name.kind() == K::attr_name => {
            Some(ColumnReference {
                qualifier: Some(first),
                column: Some(identifier(name).ok()?),
            })
        }
        [dot, star] if dot.kind() == K::Dot && star.kind() == K::Star && rest.is_empty() => {
            Some(ColumnReference {
                qualifier: Some(first),
                column: None,
            })
        }
        _ => None,
    }
}

enum RelationName {
    Table {
        schema: Option<Identifier>,
        table: Identifier,
    },
    Recovered,
}

fn relation_name(relation_expr: &Node<'_>) -> Result<RelationName, Exclusion> {
    let name = only(relation_expr, K::qualified_name)?;
    let parts = children(&name);
    // A 2way replacement comment leaves a zero-width identifier in place of the table.
    if matches!(parts.as_slice(), [col_id] if col_id.kind() == K::ColId
    && only(col_id, K::IDENT).is_ok_and(|token| {
        token.text().is_empty() && token.range().start_byte == token.range().end_byte
    })) {
        return Ok(RelationName::Recovered);
    }
    let mut names = names(&name)?;
    let table = names.pop().ok_or(Exclusion::UnsupportedSyntax)?;
    Ok(RelationName::Table {
        schema: names.pop(),
        table,
    })
}

fn names(node: &Node<'_>) -> Result<Vec<Identifier>, Exclusion> {
    let parts = children(node);
    let (first, indirection) = match parts.as_slice() {
        [first] if first.kind() == K::ColId => (first, None),
        [first, indirection]
            if first.kind() == K::ColId && indirection.kind() == K::indirection =>
        {
            (first, Some(indirection))
        }
        _ => return Err(Exclusion::UnsupportedSyntax),
    };
    let mut result = vec![identifier(first)?];
    if let Some(indirection) = indirection {
        let element = only(indirection, K::indirection_el)?;
        match children(&element).as_slice() {
            [dot, name] if dot.kind() == K::Dot && name.kind() == K::attr_name => {
                result.push(identifier(name)?)
            }
            _ => return Err(Exclusion::UnsupportedSyntax),
        }
    }
    Ok(result)
}

fn identifier(node: &Node<'_>) -> Result<Identifier, Exclusion> {
    let tokens: Vec<_> = node
        .descendants()
        .filter(|n| n.node_or_token.as_token().is_some() && !comment(n))
        .collect();
    let [token] = tokens.as_slice() else {
        return Err(Exclusion::UnsupportedIdentifier);
    };
    let spelling = token.text().to_owned();
    let name = if spelling.starts_with('"') {
        spelling
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or(Exclusion::UnsupportedIdentifier)?
            .replace("\"\"", "\"")
    } else {
        if spelling
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("u&"))
            || spelling.is_empty()
        {
            return Err(Exclusion::UnsupportedIdentifier);
        }
        spelling.to_ascii_lowercase()
    };
    if name.is_empty() || name.len() > 63 {
        return Err(Exclusion::UnsupportedIdentifier);
    }
    Ok(Identifier {
        name,
        spelling,
        range: token.range(),
    })
}

fn same_span(a: &Node<'_>, b: &Node<'_>) -> bool {
    (a.range().start_byte, a.range().end_byte) == (b.range().start_byte, b.range().end_byte)
}

fn same(a: &Node<'_>, b: &Node<'_>) -> bool {
    a.kind() == b.kind() && same_span(a, b)
}

fn comment(node: &Node<'_>) -> bool {
    matches!(node.kind(), K::C_COMMENT | K::SQL_COMMENT)
}

fn children<'a>(node: &Node<'a>) -> Vec<Node<'a>> {
    node.children()
        .into_iter()
        .filter(|n| !comment(n))
        .collect()
}

fn only<'a>(node: &Node<'a>, expected: K) -> Result<Node<'a>, Exclusion> {
    match children(node).as_slice() {
        [child] if child.kind() == expected => Ok(child.clone()),
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}
