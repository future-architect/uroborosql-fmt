use tree_sitter::TreeCursor;

use crate::{
    cst::{unary::UnaryExpr, *},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    /// IS式のフォーマットを行う。
    /// 結果を AlignedExpr で返す。
    pub(crate) fn visit_is_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let lhs = self.visit_expr(cursor, src)?;

        cursor.goto_next_sibling();
        ensure_kind(cursor, "IS")?;
        let op = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
        cursor.goto_next_sibling();

        // 右辺は "NOT" から始まる場合がある。
        // TODO: tree-sitter-sql では、右辺に distinct_from が現れるケースがあり、それには対応していない。
        let rhs = if cursor.node().kind() == "NOT" {
            let not_str = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
            let mut loc = Location::new(cursor.node().range());
            cursor.goto_next_sibling();

            let operand = self.visit_expr(cursor, src)?;
            loc.append(operand.loc());
            Expr::Unary(Box::new(UnaryExpr::new(not_str, operand, loc)))
        } else {
            self.visit_expr(cursor, src)?
        };

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op), rhs);

        cursor.goto_parent();
        ensure_kind(cursor, "is_expression")?;

        Ok(aligned)
    }
}
