use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Clause, Comment, Statement},
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor},
};

// DeleteStmt
// - opt_with_clause? DELETE_P FROM relation_expr_opt_alias using_clause? where_or_current_clause? returning_clause?
//
// opt_with_clause
// - with_clause
//
// using_clause
// - USING from_list

impl Visitor {
    pub(crate) fn visit_delete_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // DeleteStmt
        // - opt_with_clause? DELETE_P FROM relation_expr_opt_alias using_clause? where_or_current_clause? returning_clause?
        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cursor -> opt_with_clause?

        if cursor.node().kind() == SyntaxKind::opt_with_clause {
            let with_clause = self.visit_opt_with_clause(cursor, src)?;
            statement.add_clause(with_clause);

            cursor.goto_next_sibling();
        }

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> DELETE_P
        ensure_kind!(cursor, SyntaxKind::DELETE_P, src);
        let mut clause = create_clause!(cursor, SyntaxKind::DELETE_P);

        cursor.goto_next_sibling();
        self.consume_or_complement_sql_id(cursor, &mut clause);
        self.consume_comments_in_clause(cursor, &mut clause)?;

        statement.add_clause(clause);

        // cursor -> FROM
        let mut from_clause = create_clause!(cursor, SyntaxKind::FROM);
        cursor.goto_next_sibling();

        self.consume_comments_in_clause(cursor, &mut from_clause)?;

        // cursor -> relation_expr_opt_alias
        let body = self.visit_relation_expr_opt_alias(cursor, src)?;

        from_clause.set_body(body);
        statement.add_clause(from_clause);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::using_clause => {
                    let using_clause = self.visit_using_clause(cursor, src)?;
                    statement.add_clause(using_clause);
                }
                SyntaxKind::where_or_current_clause => {
                    let where_or_current_clause =
                        self.visit_where_or_current_clause(cursor, src)?;
                    statement.add_clause(where_or_current_clause);
                }
                SyntaxKind::returning_clause => {
                    let returning_clause = self.visit_returning_clause(cursor, src)?;
                    statement.add_clause(returning_clause);
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_delete_stmt(): unexpected node kind\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> DeleteStmt
        ensure_kind!(cursor, SyntaxKind::DeleteStmt, src);

        Ok(statement)
    }

    fn visit_using_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // using_clause
        // - USING from_list

        cursor.goto_first_child();

        let mut clause = create_clause!(cursor, SyntaxKind::USING);
        cursor.goto_next_sibling();

        self.consume_comments_in_clause(cursor, &mut clause)?;

        let body = self.visit_from_list(cursor, src, None)?;
        clause.set_body(body);

        cursor.goto_parent();
        // cursor -> using_clause
        ensure_kind!(cursor, SyntaxKind::using_clause, src);

        Ok(clause)
    }
}
