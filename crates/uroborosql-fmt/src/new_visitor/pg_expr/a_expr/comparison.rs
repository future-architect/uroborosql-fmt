use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, Expr},
    error::UroboroSQLFmtError,
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
    ) -> Result<Expr, UroboroSQLFmtError> {
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

        // cursor -> a_expr
        cursor.goto_next_sibling();
        let rhs = self.visit_a_expr(cursor, src)?;

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op_str), rhs);

        Ok(Expr::Aligned(Box::new(aligned)))
    }
}
