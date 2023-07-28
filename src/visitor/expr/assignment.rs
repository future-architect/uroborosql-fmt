use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    /// SET句における代入式をフォーマットする
    pub(crate) fn visit_assign_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let identifier = self.visit_expr(cursor, src)?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "=")?;
        cursor.goto_next_sibling();
        let expr = self.visit_expr(cursor, src)?;

        let mut aligned = AlignedExpr::new(identifier, false);
        aligned.add_rhs(Some("=".to_string()), expr);
        cursor.goto_parent();
        ensure_kind(cursor, "assigment_expression")?;

        Ok(aligned)
    }
}
