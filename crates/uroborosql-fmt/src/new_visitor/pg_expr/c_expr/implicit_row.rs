use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{ColumnList, Comment, Location},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, COMMA},
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

            expr_list.add_comment_to_last_item(comment)?;

            cursor.goto_next_sibling();
        }

        pg_ensure_kind!(cursor, SyntaxKind::Comma, src);
        cursor.goto_next_sibling();

        let a_expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        // expr_list に a_expr を追加
        expr_list.add_expr(a_expr.to_aligned(), Some(COMMA.to_string()));

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        // cursor -> implicit_row
        pg_ensure_kind!(cursor, SyntaxKind::implicit_row, src);

        ColumnList::try_from_expr_list(&expr_list, loc, start_comments)
    }
}
