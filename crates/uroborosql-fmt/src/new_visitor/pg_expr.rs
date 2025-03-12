mod a_expr;
mod c_expr;

use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, Comment, Expr},
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
        // cursor -> a_expr | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    exprs.push(self.visit_a_expr(cursor, src)?.to_aligned());
                }
                // バインドパラメータを想定
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // 次の式へ
                    if !cursor.goto_next_sibling() {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_expr_list(): Unexpected syntax. node: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    // cursor -> a_expr
                    pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;
                    let mut expr = self.visit_a_expr(cursor, src)?;

                    // コメントがバインドパラメータならば式に付与
                    if comment.is_block_comment() && comment.loc().is_next_to(&expr.loc()) {
                        expr.set_head_comment(comment.clone());
                    } else {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_expr_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    exprs.push(expr.to_aligned());
                }
                // 行末コメント
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_expr_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
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
