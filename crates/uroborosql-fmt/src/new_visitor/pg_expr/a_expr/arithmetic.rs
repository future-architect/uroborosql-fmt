use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{Expr, ExprSeq, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
};

use super::Visitor;

impl Visitor {
    /// 左辺の式を受け取り、算術の二項演算にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は演算子を指している
    /// 呼出後、 cursor は 最後の a_expr を指している
    ///
    pub fn handle_arithmetic_binary_expr_nodes(
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

        // cursor -> a_expr
        cursor.goto_next_sibling();
        let rhs = self.visit_a_expr(cursor, src)?;

        // 演算子を PrimaryExpr として扱う
        let op = PrimaryExpr::with_pg_node(op_node, PrimaryExprKind::Expr)?;

        // ExprSeq として返す
        let seq = ExprSeq::new(&[lhs, op.into(), rhs]);
        Ok(Expr::ExprSeq(Box::new(seq)))
    }
}
