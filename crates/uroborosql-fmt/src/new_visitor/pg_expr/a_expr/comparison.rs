use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, Comment, Expr},
    error::UroboroSQLFmtError,
    new_visitor::pg_error_annotation_from_cursor,
    CONFIG,
};

use super::Visitor;

impl Visitor {
    /// 左辺の式を受け取り、比較演算にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は演算子を指している
    /// 呼出後、 cursor は 最後の a_expr を指している
    ///
    pub fn handle_comparison_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr op a_expr
        // ^      ^  ^
        // lhs    │  │
        //        │  └ 呼出後
        //        └ 呼出前

        // cursor -> op
        let op_node = cursor.node();

        // unify_not_equalがtrueの場合は <> を != に統一する
        let op_str = if CONFIG.read().unwrap().unify_not_equal && op_node.text() == "<>" {
            "!=".to_string()
        } else {
            op_node.text().to_string()
        };

        cursor.goto_next_sibling();
        // cursor -> comment?
        // バインドパラメータを想定
        let bind_param = if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            cursor.goto_next_sibling();

            Some(comment)
        } else {
            None
        };

        // cursor -> a_expr
        let mut rhs = self.visit_a_expr_or_b_expr(cursor, src)?;

        // バインドパラメータなら付与
        if let Some(comment) = bind_param {
            if comment.is_block_comment() && comment.loc().is_next_to(&rhs.loc()) {
                rhs.set_head_comment(comment);
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_comparison_expr_nodes(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op_str), rhs);

        Ok(aligned)
    }
}
