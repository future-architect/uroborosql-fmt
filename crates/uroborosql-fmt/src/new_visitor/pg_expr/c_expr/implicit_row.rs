use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{ColumnList, Comment, Location},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    pub(crate) fn visit_implicit_row(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // implicit_row:
        // - '(' expr_list ',' a_expr ')'

        // expr_list を走査して得られた Vec<AlignedExpr> に a_expr を追加して ColumnList を作成する

        let loc = Location::from(cursor.node().range());

        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();

        // 開き括弧と最初の式との間のコメントを取得
        let mut start_comments = vec![];
        if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr_list
        let mut expr_list = self.visit_expr_list(cursor, src)?;

        cursor.goto_next_sibling();

        // cursor -> comment?
        // この位置のコメントは expr_list 最後の要素の末尾コメント
        if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());

            // expr_list は必ず1つ以上要素を持っている
            let last = expr_list.last_mut().expect("empty expr_list");

            // 末尾コメントのみを考慮する
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment)?;
            } else {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_implicit_row(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        pg_ensure_kind!(cursor, SyntaxKind::Comma, src);
        cursor.goto_next_sibling();

        let a_expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        // expr_list に a_expr を追加
        expr_list.push(a_expr.to_aligned());

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        // cursor -> implicit_row
        pg_ensure_kind!(cursor, SyntaxKind::implicit_row, src);

        Ok(ColumnList::new(expr_list, loc, start_comments))
    }
}
