//! Owned, conservative SQL input for name resolution. No parser nodes cross this boundary.

use self::expression::expr;
use crate::catalog::TableRequest;

mod expression;
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind as K,
    tree_sitter::{Node, Range},
};

#[derive(Debug, Clone)]
pub(crate) struct Identifier {
    pub name: String,
    pub spelling: String,
    pub quoted: bool,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub(crate) struct ColumnRef {
    pub qualifier: Option<Identifier>,
    pub column: Identifier,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Column(Box<ColumnRef>),
    Literal,
    Group(Box<Expr>),
    Unary {
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        right: Box<Expr>,
    },
    IsNull {
        operand: Box<Expr>,
    },
    In {
        value: Box<Expr>,
        items: Vec<Expr>,
    },
    Between {
        value: Box<Expr>,
        lower: Box<Expr>,
        upper: Box<Expr>,
    },
    Cast {
        operand: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Target {
    pub expr: Expr,
    pub alias: Option<Identifier>,
}

#[derive(Debug, Clone)]
pub(crate) enum SourceName {
    Table {
        schema: Option<Identifier>,
        table: Box<Identifier>,
    },
    Recovered,
}

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub name: SourceName,
    pub alias: Option<Identifier>,
    pub range: Range,
}

impl Source {
    pub fn request(&self) -> Option<TableRequest> {
        match &self.name {
            SourceName::Table { schema, table } => Some(TableRequest {
                schema: schema.as_ref().map(|s| s.name.clone()),
                name: table.name.clone(),
            }),
            SourceName::Recovered => None,
        }
    }
    pub fn visible_name(&self) -> Option<&str> {
        self.alias
            .as_ref()
            .map(|alias| alias.name.as_str())
            .or(match &self.name {
                SourceName::Table { table, .. } => Some(table.name.as_str()),
                SourceName::Recovered => None,
            })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Select {
    pub source: Source,
    pub targets: Vec<Target>,
    pub predicate: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exclusion {
    FileEffect,
    UnsupportedSyntax,
    UnsupportedIdentifier,
    TemporarySchema,
}

#[derive(Debug, Clone)]
pub(crate) struct Statement {
    pub range: Range,
    pub input: Result<Select, Exclusion>,
}

#[derive(Debug, Clone)]
pub(crate) struct Prepared {
    pub statements: Vec<Statement>,
    pub requests: Vec<TableRequest>,
}

pub(crate) fn extract(root: &Node<'_>) -> Prepared {
    let file_effect = root
        .children()
        .into_iter()
        .filter(|n| !comment(n) && n.kind() != K::Semicolon)
        .any(|statement| {
            statement.kind() != K::SelectStmt
                || statement.descendants().any(|n| {
                    matches!(
                        n.kind(),
                        K::into_clause
                            | K::func_application
                            | K::InsertStmt
                            | K::UpdateStmt
                            | K::DeleteStmt
                            | K::MergeStmt
                    )
                })
        });
    let mut statements = Vec::new();
    let mut requests = Vec::new();
    for node in root
        .children()
        .into_iter()
        .filter(|n| !comment(n) && n.kind() != K::Semicolon)
    {
        let range = node.range();
        let input = if file_effect {
            Err(Exclusion::FileEffect)
        } else {
            select(&node)
        };
        if let Some(request) = input
            .as_ref()
            .ok()
            .and_then(|select| select.source.request())
        {
            if !requests.contains(&request) {
                requests.push(request);
            }
        }
        statements.push(Statement { range, input });
    }
    Prepared {
        statements,
        requests,
    }
}

fn comment(node: &Node<'_>) -> bool {
    matches!(node.kind(), K::C_COMMENT | K::SQL_COMMENT)
}
fn children<'a>(node: &Node<'a>) -> Vec<Node<'a>> {
    node.children()
        .into_iter()
        .filter(|n| {
            !comment(n)
                // Recovery extras are direct children outside the grammatical lists/expressions.
                && !matches!((node.kind(), n.kind()),
                    (K::select_no_parens | K::from_clause, K::Comma)
                    | (K::where_clause, K::AND | K::OR))
        })
        .collect()
}
fn only<'a>(node: &Node<'a>, expected: K) -> Result<Node<'a>, Exclusion> {
    let c = children(node);
    match c.as_slice() {
        [child] if child.kind() == expected => Ok(child.clone()),
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}
fn kinds(nodes: &[Node<'_>], expected: &[K]) -> bool {
    nodes.len() == expected.len() && nodes.iter().zip(expected).all(|(n, k)| n.kind() == *k)
}

fn select(node: &Node<'_>) -> Result<Select, Exclusion> {
    if node.kind() != K::SelectStmt {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let body = only(node, K::select_no_parens)?;
    let c = children(&body);
    if c.first().is_none_or(|keyword| keyword.kind() != K::SELECT) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let mut next = 1;
    if c.get(next)
        .is_some_and(|part| part.kind() == K::distinct_clause)
    {
        if !kinds(&children(&c[next]), &[K::DISTINCT]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        next += 1;
    }
    let Some(target_list) = c.get(next).filter(|part| part.kind() == K::target_list) else {
        return Err(Exclusion::UnsupportedSyntax);
    };
    next += 1;
    let Some(from_clause) = c.get(next).filter(|part| part.kind() == K::from_clause) else {
        return Err(Exclusion::UnsupportedSyntax);
    };
    next += 1;
    let source = source(from_clause)?;
    let where_clause = c.get(next).filter(|part| part.kind() == K::where_clause);
    if where_clause.is_some() {
        next += 1;
    }
    let mut seen_limit = false;
    let mut seen_offset = false;
    let mut seen_locking = false;
    let trailing = &c[next..];
    for part in trailing {
        match part.kind() {
            K::limit_clause if !seen_limit => {
                limit(part)?;
                seen_limit = true;
            }
            K::offset_clause if !seen_offset => {
                offset(part)?;
                seen_offset = true;
            }
            K::for_locking_clause | K::opt_for_locking_clause if !seen_locking => {
                locking(part, &source)?;
                seen_locking = true;
            }
            _ => return Err(Exclusion::UnsupportedSyntax),
        }
    }
    let list = children(target_list);
    if list.is_empty() || list.len().is_multiple_of(2) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let mut targets = Vec::new();
    for (i, node) in list.iter().enumerate() {
        if i % 2 == 1 {
            if node.kind() != K::Comma {
                return Err(Exclusion::UnsupportedSyntax);
            }
        } else {
            if node.kind() != K::target_el {
                return Err(Exclusion::UnsupportedSyntax);
            }
            let t = children(node);
            let alias = if kinds(&t, &[K::a_expr, K::AS, K::ColLabel]) {
                let output_alias = &t[2];
                Some(identifier(output_alias)?)
            } else if kinds(&t, &[K::a_expr, K::BareColLabel]) {
                let output_alias = &t[1];
                Some(identifier(output_alias)?)
            } else if kinds(&t, &[K::a_expr]) {
                None
            } else {
                return Err(Exclusion::UnsupportedSyntax);
            };
            let target_expr = &t[0];
            targets.push(Target {
                expr: expr(target_expr)?,
                alias,
            });
        }
    }
    let predicate = if let Some(where_clause) = where_clause {
        let where_children = children(where_clause);
        if !kinds(&where_children, &[K::WHERE, K::a_expr]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        let predicate_expr = &where_children[1];
        Some(expr(predicate_expr)?)
    } else {
        None
    };
    Ok(Select {
        source,
        targets,
        predicate,
    })
}

fn integer_literal(node: &Node<'_>, expression_kind: K) -> Result<(), Exclusion> {
    let expression = only(node, expression_kind)?;
    let constant_base = if expression_kind == K::a_expr {
        only(&expression, K::c_expr)?
    } else {
        expression
    };
    let constant = only(&constant_base, K::AexprConst)?;
    let parts = children(&constant);
    match parts.as_slice() {
        [integer] if integer.kind() == K::Iconst => {
            only(integer, K::ICONST)?;
            Ok(())
        }
        [number]
            if number.kind() == K::FCONST
                && number.node_or_token.as_token().is_some()
                && integer_spelling(number.text()) =>
        {
            Ok(())
        }
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}

fn integer_spelling(text: &str) -> bool {
    let (digits, base): (&str, fn(u8) -> bool) =
        if let Some(rest) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            (rest, |digit| digit.is_ascii_hexdigit())
        } else if let Some(rest) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
            (rest, |digit| matches!(digit, b'0'..=b'7'))
        } else if let Some(rest) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
            (rest, |digit| matches!(digit, b'0' | b'1'))
        } else {
            (text, |digit| digit.is_ascii_digit())
        };
    let mut saw_digit = false;
    let mut last_was_separator = false;
    for (index, digit) in digits.bytes().enumerate() {
        if digit == b'_' {
            if last_was_separator || (index == 0 && digits.len() == text.len()) {
                return false;
            }
            last_was_separator = true;
        } else if base(digit) {
            saw_digit = true;
            last_was_separator = false;
        } else {
            return false;
        }
    }
    saw_digit && !last_was_separator
}

fn row_or_rows(node: &Node<'_>, offset: bool) -> Result<(), Exclusion> {
    let tokens = children(node);
    match tokens.as_slice() {
        [token] if token.kind() == K::ROWS || (!offset && token.kind() == K::ROW) => Ok(()),
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}

fn limit(node: &Node<'_>) -> Result<(), Exclusion> {
    let parts = children(node);
    match parts.as_slice() {
        [keyword, value] if keyword.kind() == K::LIMIT && value.kind() == K::select_limit_value => {
            integer_literal(value, K::a_expr)
        }
        [keyword, first_or_next, value, rows, only]
            if keyword.kind() == K::FETCH
                && first_or_next.kind() == K::first_or_next
                && value.kind() == K::select_fetch_first_value
                && rows.kind() == K::row_or_rows
                && only.kind() == K::ONLY =>
        {
            let first_or_next = children(first_or_next);
            if !matches!(first_or_next.as_slice(), [token] if matches!(token.kind(), K::FIRST_P | K::NEXT))
            {
                return Err(Exclusion::UnsupportedSyntax);
            }
            integer_literal(value, K::c_expr)?;
            row_or_rows(rows, false)
        }
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}

fn offset(node: &Node<'_>) -> Result<(), Exclusion> {
    let parts = children(node);
    match parts.as_slice() {
        [keyword, value]
            if keyword.kind() == K::OFFSET && value.kind() == K::select_offset_value =>
        {
            integer_literal(value, K::a_expr)
        }
        [keyword, value, rows]
            if keyword.kind() == K::OFFSET
                && value.kind() == K::select_fetch_first_value
                && rows.kind() == K::row_or_rows =>
        {
            integer_literal(value, K::c_expr)?;
            row_or_rows(rows, true)
        }
        _ => Err(Exclusion::UnsupportedSyntax),
    }
}

fn locking(node: &Node<'_>, source: &Source) -> Result<(), Exclusion> {
    let clause = if node.kind() == K::opt_for_locking_clause {
        only(node, K::for_locking_clause)?
    } else if node.kind() == K::for_locking_clause {
        node.clone()
    } else {
        return Err(Exclusion::UnsupportedSyntax);
    };
    let items = only(&clause, K::for_locking_items)?;
    let item = only(&items, K::for_locking_item)?;
    let parts = children(&item);
    let ([strength] | [strength, _]) = parts.as_slice() else {
        return Err(Exclusion::UnsupportedSyntax);
    };
    if !kinds(&children(strength), &[K::FOR, K::UPDATE]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    if let Some(locked_rels) = parts.get(1) {
        if locked_rels.kind() != K::locked_rels_list {
            return Err(Exclusion::UnsupportedSyntax);
        }
        let relations = children(locked_rels);
        if !kinds(&relations, &[K::OF, K::qualified_name_list]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        let relation = only(&relations[1], K::qualified_name)?;
        let names = names(&relation)?;
        if !matches!(names.as_slice(), [name] if Some(name.name.as_str()) == source.visible_name())
        {
            return Err(Exclusion::UnsupportedSyntax);
        }
    }
    Ok(())
}

fn source(node: &Node<'_>) -> Result<Source, Exclusion> {
    let from_children = children(node);
    if !kinds(&from_children, &[K::FROM, K::from_list]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let from_list = &from_children[1];
    let table = only(from_list, K::table_ref)?;
    let table_children = children(&table);
    if !kinds(&table_children, &[K::relation_expr])
        && !kinds(&table_children, &[K::relation_expr, K::opt_alias_clause])
    {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let relation_expr = &table_children[0];
    let relation = only(relation_expr, K::qualified_name)?;
    let parts = children(&relation);
    let recovered = matches!(parts.as_slice(), [col_id] if col_id.kind() == K::ColId
    && only(col_id, K::IDENT).is_ok_and(|token| {
        token.text().is_empty() && token.range().start_byte == token.range().end_byte
    }));
    let name = if recovered {
        SourceName::Recovered
    } else {
        match names(&relation)?.as_slice() {
            [table] => SourceName::Table {
                schema: None,
                table: Box::new(table.clone()),
            },
            [schema, table] => SourceName::Table {
                schema: Some(schema.clone()),
                table: Box::new(table.clone()),
            },
            _ => return Err(Exclusion::UnsupportedSyntax),
        }
    };
    let alias = if let Some(opt_alias_clause) = table_children.get(1) {
        let alias_clause = only(opt_alias_clause, K::alias_clause)?;
        let alias_children = children(&alias_clause);
        if kinds(&alias_children, &[K::AS, K::ColId]) {
            let alias_name = &alias_children[1];
            Some(identifier(alias_name)?)
        } else if kinds(&alias_children, &[K::ColId]) {
            let alias_name = &alias_children[0];
            Some(identifier(alias_name)?)
        } else {
            return Err(Exclusion::UnsupportedSyntax);
        }
    } else {
        None
    };
    if matches!(&name, SourceName::Table { schema: Some(schema), .. } if schema.name == "pg_temp") {
        return Err(Exclusion::TemporarySchema);
    }
    Ok(Source {
        name,
        alias,
        range: relation.range(),
    })
}

fn names(node: &Node<'_>) -> Result<Vec<Identifier>, Exclusion> {
    let name_children = children(node);
    if !kinds(&name_children, &[K::ColId]) && !kinds(&name_children, &[K::ColId, K::indirection]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let first_name = &name_children[0];
    let mut result = vec![identifier(first_name)?];
    if let Some(indirection) = name_children.get(1) {
        let element = only(indirection, K::indirection_el)?;
        let element_children = children(&element);
        if !kinds(&element_children, &[K::Dot, K::attr_name]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        let attribute_name = &element_children[1];
        result.push(identifier(&only(attribute_name, K::ColLabel)?)?);
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
    let quoted = spelling.starts_with('"');
    let name = if quoted {
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
        quoted,
        range: token.range(),
    })
}

#[cfg(test)]
mod tests;
