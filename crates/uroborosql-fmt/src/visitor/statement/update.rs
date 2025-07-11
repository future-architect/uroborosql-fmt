use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Comment, Statement},
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor},
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

        // cursor -> UPDATE
        ensure_kind!(cursor, SyntaxKind::UPDATE, src);
        let mut update_clause = create_clause!(cursor, SyntaxKind::UPDATE);

        cursor.goto_next_sibling();
        self.consume_or_complement_sql_id(cursor, &mut update_clause);
        self.consume_comments_in_clause(cursor, &mut update_clause)?;

        // cursor -> relation_expr_opt_alias
        let body = self.visit_relation_expr_opt_alias(cursor, src)?;
        update_clause.set_body(body);
        statement.add_clause(update_clause);

        cursor.goto_next_sibling();

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> SET
        ensure_kind!(cursor, SyntaxKind::SET, src);
        let mut set_clause = create_clause!(cursor, SyntaxKind::SET);
        cursor.goto_next_sibling();

        // キーワード直後のコメントを処理
        self.consume_comments_in_clause(cursor, &mut set_clause)?;

        // cursor -> set_clause_list
        ensure_kind!(cursor, SyntaxKind::set_clause_list, src);
        let set_clause_list = self.visit_set_clause_list(cursor, src)?;

        set_clause.set_body(set_clause_list);
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
                    let comment = Comment::new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_update_stmt(): unexpected syntax\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> UpdateStmt
        ensure_kind!(cursor, SyntaxKind::UpdateStmt, src);

        Ok(statement)
    }
}
