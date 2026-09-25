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
    if !kinds(&c, &[K::SELECT, K::target_list, K::from_clause])
        && !kinds(
            &c,
            &[K::SELECT, K::target_list, K::from_clause, K::where_clause],
        )
    {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let target_list = &c[1];
    let from_clause = &c[2];
    let where_clause = c.get(3);
    let source = source(from_clause)?;
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

fn expr(node: &Node<'_>) -> Result<Expr, Exclusion> {
    let c = children(node);
    match node.kind() {
        K::a_expr => {
            if kinds(&c, &[K::c_expr]) {
                let primary_expr = &c[0];
                return expr(primary_expr);
            }
            if let [operator, operand] = c.as_slice() {
                if matches!(operator.kind(), K::Plus | K::Minus | K::NOT)
                    && operand.kind() == K::a_expr
                {
                    return Ok(Expr::Unary {
                        operand: Box::new(expr(operand)?),
                    });
                }
            }
            if let [left, operator, right] = c.as_slice() {
                if left.kind() == K::a_expr
                    && right.kind() == K::a_expr
                    && matches!(
                        operator.kind(),
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
                        left: Box::new(expr(left)?),
                        right: Box::new(expr(right)?),
                    });
                }
            }
            if kinds(&c, &[K::a_expr, K::IS, K::NULL_P])
                || kinds(&c, &[K::a_expr, K::IS, K::NOT, K::NULL_P])
            {
                let operand = &c[0];
                return Ok(Expr::IsNull {
                    operand: Box::new(expr(operand)?),
                });
            }
        }
        K::c_expr => {
            if kinds(&c, &[K::columnref]) {
                let column_ref = &c[0];
                let names = names(column_ref)?;
                let (qualifier, column) = match names.as_slice() {
                    [col] => (None, col.clone()),
                    [q, col] => (Some(q.clone()), col.clone()),
                    _ => return Err(Exclusion::UnsupportedSyntax),
                };
                return Ok(Expr::Column(Box::new(ColumnRef {
                    qualifier,
                    column,
                    range: column_ref.range(),
                })));
            }
            if kinds(&c, &[K::LParen, K::a_expr, K::RParen]) {
                let grouped_expr = &c[1];
                return Ok(Expr::Group(Box::new(expr(grouped_expr)?)));
            }
            if kinds(&c, &[K::AexprConst]) {
                let constant = &c[0];
                let literal = children(constant);
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
