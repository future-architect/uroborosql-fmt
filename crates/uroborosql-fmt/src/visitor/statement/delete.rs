use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMENT},
};

impl Visitor {
    /// DELETE文をStatement構造体で返す
    pub(crate) fn visit_delete_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cusor -> with_clause?

        if cursor.node().kind() == "with_clause" {
            // with句を追加する
            let mut with_clause = self.visit_with_clause(cursor, src)?;
            cursor.goto_next_sibling();
            // with句の後に続くコメントを消費する
            self.consume_comment_in_clause(cursor, src, &mut with_clause)?;

            statement.add_clause(with_clause);
        }

        // cursor -> delete_clause
        ensure_kind(cursor, "DELETE", src)?;

        // DELETE
        let mut clause = create_clause(cursor, src, "DELETE")?;
        cursor.goto_next_sibling();
        self.consume_or_complement_sql_id(cursor, src, &mut clause);
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        statement.add_clause(clause);

        // cursor -> from_clause
        let from_clause = self.visit_from_clause(cursor, src)?;
        statement.add_clause(from_clause);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "where_clause" => {
                    let clause = self.visit_where_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                "returning_clause" => {
                    let clause =
                        self.visit_simple_clause(cursor, src, "returning_clause", "RETURNING")?;
                    statement.add_clause(clause);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_delete_stmt(): unimplemented delete_statement\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "delete_statement", src)?;

        Ok(statement)
    }
}
