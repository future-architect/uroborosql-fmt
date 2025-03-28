use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{Comment, Expr, SeparatedLines},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
};

use super::Visitor;

impl Visitor {
    /// 左辺の式を受け取り、論理演算（AND, OR）にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は演算子を指している
    /// 呼出後、 cursor は 最後の a_expr を指している
    ///
    pub fn handle_logical_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<SeparatedLines, UroboroSQLFmtError> {
        // a_expr AND/OR a_expr
        // ^      ^      ^
        // lhs    │      │
        //        │      └ 呼出後
        //        └ 呼出前

        let mut boolean_expr = SeparatedLines::new();

        // 左辺がBooleanの場合は初期化したBooleanExprを左辺で上書き
        match lhs {
            Expr::Boolean(boolean) => boolean_expr = *boolean,
            _ => boolean_expr.add_expr(lhs.to_aligned(), None, vec![]),
        }

        // cursor -> COMMENT | op

        while cursor.node().is_comment() {
            boolean_expr.add_comment_to_child(Comment::pg_new(cursor.node()))?;
            cursor.goto_next_sibling();
        }

        let sep = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> _expression

        let mut comments = vec![];
        while cursor.node().is_comment() {
            comments.push(Comment::pg_new(cursor.node()));
            cursor.goto_next_sibling();
        }

        let right = self.visit_a_expr_or_b_expr(cursor, src)?;

        if let Expr::Boolean(boolean) = right {
            // 右辺がbooleanの場合はマージ処理を行う
            boolean_expr.merge_boolean_expr(sep, *boolean);
        } else {
            boolean_expr.add_expr(right.to_aligned(), Some(sep), comments);
        }

        Ok(boolean_expr)
    }
}
