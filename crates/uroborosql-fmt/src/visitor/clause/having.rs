use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause},
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, Visitor},
};

impl Visitor {
    pub(crate) fn visit_having_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // having_clause
        // - HAVING a_expr

        cursor.goto_first_child();

        let mut clause = create_clause!(cursor, SyntaxKind::HAVING);
        cursor.goto_next_sibling();
        self.consume_comments_in_clause(cursor, &mut clause)?;

        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        let body = Body::from(expr);
        clause.set_body(body);

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::having_clause, src);

        Ok(clause)
    }
}
