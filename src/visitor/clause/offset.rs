use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    /// OFFSET句をClause構造体で返す
    /// SELECT文で使用する
    pub(crate) fn visit_offset_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();
        ensure_kind(cursor, "OFFSET")?;

        let mut offset_clause = Clause::from_node(cursor.node(), src);

        cursor.goto_next_sibling();
        // cursor -> number

        // numberをExprに格納
        let number = self.visit_expr(cursor, src)?;

        // numberからBody::SingleLineを作成
        let body = Body::SingleLine(Box::new(SingleLine::new(number)));

        offset_clause.set_body(body);

        cursor.goto_parent();
        ensure_kind(cursor, "offset_clause")?;

        Ok(offset_clause)
    }
}
