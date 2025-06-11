use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Comment},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
    NewVisitor as Visitor,
};

impl Visitor {
    pub fn visit_where_or_current_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // where_or_current_clause
        // - WHERE a_expr
        // - WHERE CURRENT_P OF cursor_name

        cursor.goto_first_child();

        let mut clause = pg_create_clause!(cursor, SyntaxKind::WHERE);
        cursor.goto_next_sibling();

        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        match cursor.node().kind() {
            SyntaxKind::a_expr => {
                let a_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
                clause.set_body(Body::from(a_expr));
            }
            SyntaxKind::CURRENT_P => {
                // CURRENT_P OF cursor_name
                //
                // cursor_name
                // - name
                // - ColId
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_where_or_current_clause(): WHERE CURRENT_P OF cursor_name is not implemented.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                let comment = Comment::pg_new(cursor.node());
                clause.add_comment_to_child(comment)?;
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_where_or_current_clause(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_parent();
        // cursor -> where_or_current_clause
        pg_ensure_kind!(cursor, SyntaxKind::where_or_current_clause, src);

        Ok(clause)
    }
}
