//! 副問い合わせに関する式のフォーマットを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    formatter::{ensure_kind, Formatter, COMMENT},
};

impl Formatter {
    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursorはselect_subexpressionを指している
    pub(crate) fn format_select_subexpr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SelectSubExpr, UroboroSQLFmtError> {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(cursor.node().range());

        // cursor -> select_subexpression

        cursor.goto_first_child();
        // cursor -> (

        cursor.goto_next_sibling();
        // cursor -> comments | select_statement

        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_statement
        let mut select_stmt = self.format_select_stmt(cursor, src)?;

        // select_statementの前にコメントがあった場合、コメントを追加
        comment_buf
            .into_iter()
            .for_each(|c| select_stmt.add_comment(c));

        cursor.goto_next_sibling();
        // cursor -> comments | )

        while cursor.node().kind() == COMMENT {
            // 閉じかっこの直前にコメントが来る場合
            let comment = Comment::new(cursor.node(), src);
            select_stmt.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> )
        cursor.goto_parent();
        ensure_kind(cursor, "select_subexpression")?;

        Ok(SelectSubExpr::new(select_stmt, loc))
    }
}
