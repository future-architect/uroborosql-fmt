use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{unary::UnaryExpr, Expr, Location},
    error::UroboroSQLFmtError,
};

use super::Visitor;

/// 単項演算子の処理
///
/// 呼出時、 cursor は演算子ノードを指している
/// 呼出後、 cursor は a_expr を指している
/// op a_expr
/// ^  ^
/// │  │
/// │  └ 呼出後
/// └ 呼出前
///
impl Visitor {
    pub fn handle_unary_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // op a_expr

        // cursor -> op
        let operator = cursor.node().text();
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> a_expr

        let operand = self.visit_a_expr(cursor, src)?;
        loc.append(operand.loc());

        Ok(Expr::Unary(Box::new(UnaryExpr::new(
            operator, operand, loc,
        ))))
    }
}
