use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        Comment,
        Statement,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
    NewVisitor as Visitor,
};

// UpdateStmt:
// - opt_with_clause? UPDATE relation_expr_opt_alias SET set_clause_list from_clause? where_or_current_clause? returning_clause?
//
// opt_with_clause:
// - with_clause

impl Visitor {
    pub(crate) fn visit_update_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cursor -> opt_with_clause?

        if cursor.node().kind() == SyntaxKind::opt_with_clause {
            // opt_with_clause
            // - with_clause

            cursor.goto_first_child();
            pg_ensure_kind!(cursor, SyntaxKind::with_clause, src);

            let with_clause = self.visit_with_clause(cursor, src)?;

            statement.add_clause(with_clause);

            cursor.goto_parent();
            pg_ensure_kind!(cursor, SyntaxKind::opt_with_clause, src);

            cursor.goto_next_sibling();
        }

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> UPDATE
        pg_ensure_kind!(cursor, SyntaxKind::UPDATE, src);
        let mut update_clause = pg_create_clause!(cursor, SyntaxKind::UPDATE);

        cursor.goto_next_sibling();
        self.pg_consume_or_complement_sql_id(cursor, &mut update_clause);
        self.pg_consume_comments_in_clause(cursor, &mut update_clause)?;

        // cursor -> relation_expr_opt_alias
        let body = self.visit_relation_expr_opt_alias(cursor, src)?;
        update_clause.set_body(body);
        statement.add_clause(update_clause);

        cursor.goto_next_sibling();

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        let set_clause = self.handle_set_clause_nodes(cursor, src)?;
        statement.add_clause(set_clause);

        // from_clause, where_or_current_clause, returning_clause を持つ可能性がある
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::from_clause => {
                    let clause = self.visit_from_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::where_or_current_clause => {
                    let clause = self.visit_where_or_current_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::returning_clause => {
                    let clause = self.visit_returning_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_update_stmt(): unexpected syntax\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> UpdateStmt
        pg_ensure_kind!(cursor, SyntaxKind::UpdateStmt, src);

        Ok(statement)
    }
}
