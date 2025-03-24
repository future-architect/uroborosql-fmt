use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Location, SubExpr},
    error::UroboroSQLFmtError,
    new_visitor::pg_ensure_kind,
};

use super::Visitor;

impl Visitor {
    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursor は select_subexpression を指している
    pub fn visit_select_with_parens(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SubExpr, UroboroSQLFmtError> {
        // select_with_parens
        // - '(' select_no_parens ')'
        // - '(' select_with_parens ')'
        //
        // select_no_parens というノードは実際には存在しない（cst-parser で消去される）
        // そのため、かっこの中に通常の select 文の要素が並ぶと考えればよい

        let loc = Location::from(cursor.node().range());

        // cursor -> select_with_parens

        cursor.goto_first_child();
        // cursor -> '('

        cursor.goto_next_sibling();

        // cursor -> comments?
        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> SELECT keyword

        // SelectStmt の子要素にあたるノード群が並ぶ
        // 呼出し後、cursor は ')' を指す
        let mut select_stmt = self.visit_select_stmt_inner(cursor, src)?;

        // select 文の前にコメントがあった場合、コメントを追加
        comment_buf
            .into_iter()
            .for_each(|c| select_stmt.add_comment(c));

        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::select_with_parens, src)?;

        Ok(SubExpr::new(select_stmt, loc))
    }
}
