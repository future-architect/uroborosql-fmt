use tree_sitter::TreeCursor;

use crate::{
    cst::{unary::UnaryExpr, Location},
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    pub(crate) fn visit_unary_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<UnaryExpr, UroboroSQLFmtError> {
        // cursor -> unary_expression

        let mut loc = Location::new(cursor.node().range());

        cursor.goto_first_child();
        // cursor -> op ("+", "-", "!!", "~", "@", "|/", "||/", "NOT")
        let operator = cursor.node().utf8_text(src.as_bytes()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> _expression

        let operand = self.visit_expr(cursor, src)?;
        loc.append(operand.loc());

        cursor.goto_parent();
        ensure_kind(cursor, "unary_expression", src)?;

        Ok(UnaryExpr::new(operator, operand, loc))
    }
}
