use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    formatter::{ensure_kind, Formatter},
};

use super::is_comp_op;

impl Formatter {
    pub(crate) fn format_binary_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // cursor -> binary_expression

        cursor.goto_first_child();
        // cursor -> _expression

        // 左辺
        let lhs_expr = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> op (e.g., "+", "-", "=", ...)

        // 演算子
        let op_node = cursor.node();
        let op_str = op_node.utf8_text(src.as_ref()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> _expression

        // 右辺
        let rhs_expr = self.format_expr(cursor, src)?;

        // cursorを戻しておく
        cursor.goto_parent();
        ensure_kind(cursor, "binary_expression")?;

        if is_comp_op(op_str) {
            // 比較演算子ならばそろえる必要があるため、AlignedExprとする
            let mut aligned = AlignedExpr::new(lhs_expr, false);
            aligned.add_rhs(op_str, rhs_expr);

            Ok(Expr::Aligned(Box::new(aligned)))
        } else {
            // 比較演算子でないならば、PrimaryExprに
            // e.g.,) 1 + 1
            match lhs_expr {
                Expr::Primary(mut lhs) => {
                    lhs.add_element(op_str);
                    match rhs_expr {
                        Expr::Primary(rhs) => {
                            lhs.append(*rhs);
                            Ok(Expr::Primary(lhs))
                        }
                        _ => {
                            // 右辺が複数行の場合
                            // todo
                            Err(UroboroSQLFmtError::UnimplementedError(format!(
                                "format_binary_expr(): (binary expression) right has multiple lines \nnode_kind: {}\n{:#?}",
                                cursor.node().kind(),
                                cursor.node().range(),
                            )))
                        }
                    }
                }
                _ => {
                    // 左辺が複数行の場合
                    // todo
                    Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_expr(): (binary expression) left has multiple lines \nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )))
                }
            }
        }
    }
}
