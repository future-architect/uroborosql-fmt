mod a_expr;
mod c_expr;

use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, Expr},
    error::UroboroSQLFmtError,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};

impl Visitor {
    fn visit_b_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        unimplemented!()
    }

    /// 呼出し後、 cursor は expr_list を指している
    pub(crate) fn visit_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // cursor -> expr_list
        pg_ensure_kind(cursor, SyntaxKind::expr_list, src)?;

        cursor.goto_first_child();
        // cursor -> a_expr

        let mut exprs = Vec::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            exprs.push(self.visit_a_expr(cursor, src)?.to_aligned());
        }

        // 残りの要素
        // cursor -> a_expr | Comma
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    exprs.push(self.visit_a_expr(cursor, src)?.to_aligned());
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_expr_list(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> expr_list

        Ok(exprs)
    }
}
