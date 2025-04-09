use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause},
    error::UroboroSQLFmtError,
    pg_create_clause, pg_ensure_kind, NewVisitor as Visitor,
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

        let mut clause = pg_create_clause!(cursor, SyntaxKind::HAVING);
        cursor.goto_next_sibling();
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        let body = Body::from(expr);
        clause.set_body(body);

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::having_clause, src);

        Ok(clause)
    }
}
