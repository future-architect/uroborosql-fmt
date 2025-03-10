use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{unary::UnaryExpr, AlignedExpr, Expr, Location, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
};

use super::super::pg_error_annotation_from_cursor;
use super::Visitor;

impl Visitor {
    /// 左辺の式を受け取り、IS述語にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は IS キーワードを指している
    /// 呼出後、 cursor は 最後のノード（NULL_P, TRUE_P, FALSE_P など）を指している
    ///
    pub fn handle_is_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // a_expr IS [NOT] NULL/TRUE/FALSE/...
        // ^      ^        ^
        // lhs    │        │
        //        │        └ 呼出後
        //        │
        //        └ 呼出前

        // cursor -> IS
        let op = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> NOT?

        let not_node = if cursor.node().kind() == SyntaxKind::NOT {
            let not_node = cursor.node();
            cursor.goto_next_sibling();

            Some(not_node)
        } else {
            None
        };

        // cursor -> NULL_P | TRUE_P | FALSE_P | DISTINCT | UNKNOWN | DOCUMENT_P | NORMALIZED | unicode_normal_form | json_predicate_type_constraint
        let last_expr = match cursor.node().kind() {
            SyntaxKind::NULL_P | SyntaxKind::TRUE_P | SyntaxKind::FALSE_P => {
                let primary = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
                Expr::Primary(Box::new(primary))
            }
            SyntaxKind::DISTINCT => {
                // IS NOT? DISTINCT FROM

                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_is_expr_nodes(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::UNKNOWN => {
                // IS NOT? UNKNOWN

                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_is_expr_nodes(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::DOCUMENT_P
            | SyntaxKind::NORMALIZED
            | SyntaxKind::unicode_normal_form
            | SyntaxKind::json_predicate_type_constraint => {
                // - IS NOT? DOCUMENT_P
                // - IS NOT? unicode_normal_form? NORMALIZED
                // - IS NOT? json_predicate_type_constraint json_key_uniqueness_constraint_opt

                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_is_expr_nodes(): {} is not implemented.\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_is_expr_nodes(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        // AlignedExpr の右辺
        // NOT がある場合は NOT を演算子とした UnaryExpr にする
        let rhs = if let Some(not_node) = not_node {
            let mut loc = Location::from(not_node.range());
            loc.append(last_expr.loc());

            let unary = UnaryExpr::new(not_node.text(), last_expr, loc);
            Expr::Unary(Box::new(unary))
        } else {
            last_expr
        };

        // IS を演算子とした AlignedExpr
        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op), rhs);

        Ok(Expr::Aligned(Box::new(aligned)))
    }
}
