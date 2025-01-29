use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{create_clause, ensure_kind, Visitor},
};

impl Visitor {
    /// HAVING句をClauseで返す
    pub(crate) fn visit_having_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let mut clause = create_clause(cursor, src, "HAVING")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> _expression
        let expr = self.visit_expr(cursor, src)?;

        // 結果として得られた式をBodyに変換する
        let body = Body::from(expr);

        clause.set_body(body);

        // cursorを戻す
        cursor.goto_parent();
        ensure_kind(cursor, "having_clause", src)?;

        Ok(clause)
    }
}
