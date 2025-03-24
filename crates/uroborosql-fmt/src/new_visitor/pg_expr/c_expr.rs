mod aexpr_const;
mod case_expr;
mod columnref;
mod func_expr;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Expr, ParenExpr},
    error::UroboroSQLFmtError,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};

/*
 * c_expr の構造
 *
 * - columnref
 * - AexprConst
 * - PARAM opt_indirection
 * - '(' a_expr ')' opt_indirection
 * - case_expr
 * - func_expr
 * - select_with_parens
 * - select_with_parens indirection
 * - EXISTS select_with_parens
 * - ARRAY select_with_parens
 * - ARRAY array_expr
 * - explicit_row
 * - implicit_row
 * - GROUPING '(' expr_list ')'
 */

impl Visitor {
    pub fn visit_c_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::columnref => self.visit_columnref(cursor, src)?,
            SyntaxKind::AexprConst => self.visit_aexpr_const(cursor, src)?,
            SyntaxKind::PARAM => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): PARAM is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::LParen => {
                // '(' a_expr ')' opt_indirection

                // cursor -> '('
                pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

                cursor.goto_next_sibling();
                // cursor -> comments?

                let mut start_comment_buf = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    start_comment_buf.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> expr
                pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;
                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
                // TODO: remove_redundant_nest

                cursor.goto_next_sibling();

                // cursor -> comments?
                let mut end_comment_buf = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    end_comment_buf.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> ")"
                pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

                // 親(c_expr) の location を設定
                // 親が無いことはありえないので、parent() の返り値が None の場合は panic する
                let parent = cursor
                    .node()
                    .parent()
                    .expect("visit_c_expr(): parent is None");

                // 親の location を設定
                let mut paren_expr = ParenExpr::new(expr, parent.range().into());

                // 開きかっこと式の間にあるコメントを追加
                for comment in start_comment_buf {
                    paren_expr.add_start_comment(comment);
                }

                // 式から閉じかっこの間にあるコメントを追加
                for comment in end_comment_buf {
                    paren_expr.add_end_comment(comment);
                }

                Expr::ParenExpr(Box::new(paren_expr))
            }
            SyntaxKind::case_expr => {
                let cond_expr = self.visit_case_expr(cursor, src)?;
                Expr::Cond(Box::new(cond_expr))
            }
            SyntaxKind::func_expr => self.visit_func_expr(cursor, src)?,
            SyntaxKind::select_with_parens => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): select_with_parens is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::EXISTS => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): EXISTS is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::ARRAY => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): ARRAY is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::explicit_row => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): explicit_row is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::implicit_row => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): implicit_row is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::GROUPING => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_c_expr(): GROUPING is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_c_expr(): unexpected syntaxkind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::c_expr, src)?;

        Ok(expr)
    }
}
