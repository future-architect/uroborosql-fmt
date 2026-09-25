//! Owned, conservative SQL input for name resolution. No parser nodes cross this boundary.

use crate::catalog::TableRequest;
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
    Unary { operand: Box<Expr> },
    Binary { left: Box<Expr>, right: Box<Expr> },
    IsNull { operand: Box<Expr> },
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
    SessionChange,
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
    let session_change = root.descendants().any(|n| {
        matches!(
            n.kind(),
            K::VariableSetStmt | K::VariableResetStmt | K::DiscardStmt
        )
    });
    let mut statements = Vec::new();
    let mut requests = Vec::new();
    for node in root
        .children()
        .into_iter()
        .filter(|n| !comment(n) && n.kind() != K::Semicolon)
    {
        let range = node.range();
        let input = if session_change {
            Err(Exclusion::SessionChange)
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
    if !kinds(&c, &[K::SELECT, K::target_list, K::from_clause])
        && !kinds(
            &c,
            &[K::SELECT, K::target_list, K::from_clause, K::where_clause],
        )
    {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let source = source(&c[2])?;
    if matches!(&source.name, SourceName::Table { schema: Some(schema), .. } if schema.name == "pg_temp")
    {
        return Err(Exclusion::TemporarySchema);
    }
    let list = children(&c[1]);
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
                Some(identifier(&t[2])?)
            } else if kinds(&t, &[K::a_expr]) {
                None
            } else {
                return Err(Exclusion::UnsupportedSyntax);
            };
            targets.push(Target {
                expr: expr(&t[0])?,
                alias,
            });
        }
    }
    let predicate = if let Some(w) = c.get(3) {
        let w = children(w);
        if !kinds(&w, &[K::WHERE, K::a_expr]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        Some(expr(&w[1])?)
    } else {
        None
    };
    Ok(Select {
        source,
        targets,
        predicate,
    })
}

fn source(node: &Node<'_>) -> Result<Source, Exclusion> {
    let c = children(node);
    if !kinds(&c, &[K::FROM, K::from_list]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let table = only(&c[1], K::table_ref)?;
    let t = children(&table);
    if !kinds(&t, &[K::relation_expr]) && !kinds(&t, &[K::relation_expr, K::opt_alias_clause]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let relation = only(&t[0], K::qualified_name)?;
    let parts = children(&relation);
    let recovered = parts.len() == 1
        && parts[0].kind() == K::ColId
        && only(&parts[0], K::IDENT).is_ok_and(|token| {
            token.text().is_empty() && token.range().start_byte == token.range().end_byte
        });
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
    let alias = if let Some(a) = t.get(1) {
        let a = only(a, K::alias_clause)?;
        let a = children(&a);
        if kinds(&a, &[K::AS, K::ColId]) {
            Some(identifier(&a[1])?)
        } else if kinds(&a, &[K::ColId]) {
            Some(identifier(&a[0])?)
        } else {
            return Err(Exclusion::UnsupportedSyntax);
        }
    } else {
        None
    };
    Ok(Source {
        name,
        alias,
        range: relation.range(),
    })
}

fn names(node: &Node<'_>) -> Result<Vec<Identifier>, Exclusion> {
    let c = children(node);
    if !kinds(&c, &[K::ColId]) && !kinds(&c, &[K::ColId, K::indirection]) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let mut result = vec![identifier(&c[0])?];
    if let Some(indirection) = c.get(1) {
        let element = only(indirection, K::indirection_el)?;
        let e = children(&element);
        if !kinds(&e, &[K::Dot, K::attr_name]) {
            return Err(Exclusion::UnsupportedSyntax);
        }
        result.push(identifier(&only(&e[1], K::ColLabel)?)?);
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

fn expr(node: &Node<'_>) -> Result<Expr, Exclusion> {
    let c = children(node);
    match node.kind() {
        K::a_expr => {
            if kinds(&c, &[K::c_expr]) {
                return expr(&c[0]);
            }
            if c.len() == 2
                && matches!(c[0].kind(), K::Plus | K::Minus | K::NOT)
                && c[1].kind() == K::a_expr
            {
                return Ok(Expr::Unary {
                    operand: Box::new(expr(&c[1])?),
                });
            }
            if c.len() == 3
                && c[0].kind() == K::a_expr
                && c[2].kind() == K::a_expr
                && matches!(
                    c[1].kind(),
                    K::Plus
                        | K::Minus
                        | K::Star
                        | K::Slash
                        | K::Equals
                        | K::NOT_EQUALS
                        | K::Less
                        | K::Greater
                        | K::LESS_EQUALS
                        | K::GREATER_EQUALS
                        | K::AND
                        | K::OR
                )
            {
                return Ok(Expr::Binary {
                    left: Box::new(expr(&c[0])?),
                    right: Box::new(expr(&c[2])?),
                });
            }
            if kinds(&c, &[K::a_expr, K::IS, K::NULL_P])
                || kinds(&c, &[K::a_expr, K::IS, K::NOT, K::NULL_P])
            {
                return Ok(Expr::IsNull {
                    operand: Box::new(expr(&c[0])?),
                });
            }
        }
        K::c_expr => {
            if kinds(&c, &[K::columnref]) {
                let names = names(&c[0])?;
                let (qualifier, column) = match names.as_slice() {
                    [col] => (None, col.clone()),
                    [q, col] => (Some(q.clone()), col.clone()),
                    _ => return Err(Exclusion::UnsupportedSyntax),
                };
                return Ok(Expr::Column(Box::new(ColumnRef {
                    qualifier,
                    column,
                    range: c[0].range(),
                })));
            }
            if kinds(&c, &[K::LParen, K::a_expr, K::RParen]) {
                return Ok(Expr::Group(Box::new(expr(&c[1])?)));
            }
            if kinds(&c, &[K::AexprConst]) {
                let literal = children(&c[0]);
                if literal.len() == 1 {
                    let atom = &literal[0];
                    let valid = match atom.kind() {
                        K::TRUE_P | K::FALSE_P | K::NULL_P | K::FCONST => {
                            atom.node_or_token.as_token().is_some()
                        }
                        K::Iconst => only(atom, K::ICONST).is_ok(),
                        K::Sconst => only(atom, K::SCONST).is_ok(),
                        _ => false,
                    };
                    if valid {
                        return Ok(Expr::Literal);
                    }
                }
            }
        }
        _ => {}
    }
    Err(Exclusion::UnsupportedSyntax)
}

#[cfg(test)]
mod tests;
