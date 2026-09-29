use super::{children, kinds, names, only, ColumnRef, Exclusion, Expr};
use postgresql_cst_parser::{syntax_kind::SyntaxKind as K, tree_sitter::Node};

fn in_items(node: &Node<'_>) -> Result<Vec<Expr>, Exclusion> {
    let parts = children(node);
    let [open, expression_list, close] = parts.as_slice() else {
        return Err(Exclusion::UnsupportedSyntax);
    };
    if open.kind() != K::LParen
        || expression_list.kind() != K::expr_list
        || close.kind() != K::RParen
    {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let list = children(expression_list);
    if list.is_empty() || list.len().is_multiple_of(2) {
        return Err(Exclusion::UnsupportedSyntax);
    }
    let mut items = Vec::new();
    for (index, item) in list.iter().enumerate() {
        if index.is_multiple_of(2) {
            if item.kind() != K::a_expr {
                return Err(Exclusion::UnsupportedSyntax);
            }
            items.push(expr(item)?);
        } else if item.kind() != K::Comma {
            return Err(Exclusion::UnsupportedSyntax);
        }
    }
    Ok(items)
}

fn simple_type(node: &Node<'_>) -> Result<(), Exclusion> {
    let simple = only(node, K::SimpleTypename)?;
    let family = children(&simple);
    if !matches!(family.as_slice(), [kind] if matches!(kind.kind(),
        K::GenericType | K::Numeric | K::Character | K::Bit | K::ConstDatetime | K::ConstInterval))
        || simple.descendants().any(|part| {
            matches!(
                part.kind(),
                K::attrs
                    | K::opt_type_modifiers
                    | K::CharacterWithLength
                    | K::LParen
                    | K::RParen
                    | K::LBracket
                    | K::RBracket
                    | K::Dot
                    | K::Comma
            )
        })
    {
        return Err(Exclusion::UnsupportedSyntax);
    }
    Ok(())
}

pub(super) fn expr(node: &Node<'_>) -> Result<Expr, Exclusion> {
    let c = children(node);
    match node.kind() {
        K::a_expr | K::b_expr => {
            let expression_kind = node.kind();
            if kinds(&c, &[K::c_expr]) {
                let primary_expr = &c[0];
                return expr(primary_expr);
            }
            if let [operator, operand] = c.as_slice() {
                if matches!(operator.kind(), K::Plus | K::Minus | K::NOT)
                    && operand.kind() == expression_kind
                {
                    return Ok(Expr::Unary {
                        operand: Box::new(expr(operand)?),
                    });
                }
            }
            if let [left, operator, right] = c.as_slice() {
                if left.kind() == expression_kind
                    && right.kind() == expression_kind
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
            if kinds(&c, &[K::a_expr, K::IN_P, K::in_expr])
                || kinds(&c, &[K::a_expr, K::NOT_LA, K::IN_P, K::in_expr])
            {
                let value = &c[0];
                let in_expression = c.last().expect("checked IN shape");
                let items = in_items(in_expression)?;
                return Ok(Expr::In {
                    value: Box::new(expr(value)?),
                    items,
                });
            }
            if kinds(&c, &[K::a_expr, K::BETWEEN, K::b_expr, K::AND, K::a_expr])
                || kinds(
                    &c,
                    &[
                        K::a_expr,
                        K::NOT_LA,
                        K::BETWEEN,
                        K::b_expr,
                        K::AND,
                        K::a_expr,
                    ],
                )
            {
                let (value, lower, upper) = match c.as_slice() {
                    [value, _, lower, _, upper] | [value, _, _, lower, _, upper] => {
                        (value, lower, upper)
                    }
                    _ => unreachable!("checked BETWEEN shape"),
                };
                return Ok(Expr::Between {
                    value: Box::new(expr(value)?),
                    lower: Box::new(expr(lower)?),
                    upper: Box::new(expr(upper)?),
                });
            }
            if kinds(&c, &[K::a_expr, K::LIKE, K::a_expr])
                || kinds(&c, &[K::a_expr, K::ILIKE, K::a_expr])
                || kinds(&c, &[K::a_expr, K::NOT_LA, K::LIKE, K::a_expr])
                || kinds(&c, &[K::a_expr, K::NOT_LA, K::ILIKE, K::a_expr])
            {
                let left = &c[0];
                let right = c.last().expect("checked LIKE shape");
                return Ok(Expr::Binary {
                    left: Box::new(expr(left)?),
                    right: Box::new(expr(right)?),
                });
            }
            if let [left, operator, right] = c.as_slice() {
                if left.kind() == expression_kind
                    && operator.kind() == K::qual_Op
                    && right.kind() == expression_kind
                    && only(operator, K::Op).is_ok_and(|op| op.text() == "||")
                {
                    return Ok(Expr::Binary {
                        left: Box::new(expr(left)?),
                        right: Box::new(expr(right)?),
                    });
                }
            }
            if kinds(&c, &[expression_kind, K::TYPECAST, K::Typename]) {
                let value = &c[0];
                let type_name = &c[2];
                simple_type(type_name)?;
                return Ok(Expr::Cast {
                    operand: Box::new(expr(value)?),
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
            if kinds(&c, &[K::func_expr]) {
                let function = &c[0];
                let common = only(function, K::func_expr_common_subexpr)?;
                let cast = children(&common);
                if let [cast_keyword, open, value, as_keyword, type_name, close] = cast.as_slice() {
                    if cast_keyword.kind() != K::CAST
                        || open.kind() != K::LParen
                        || value.kind() != K::a_expr
                        || as_keyword.kind() != K::AS
                        || type_name.kind() != K::Typename
                        || close.kind() != K::RParen
                    {
                        return Err(Exclusion::UnsupportedSyntax);
                    }
                    simple_type(type_name)?;
                    return Ok(Expr::Cast {
                        operand: Box::new(expr(value)?),
                    });
                }
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
