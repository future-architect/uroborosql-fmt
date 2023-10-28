use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{create_clause, create_error_info, ensure_kind, Visitor, COMMENT},
};

impl Visitor {
    /// UPDATE文をStatement構造体で返す
    pub(crate) fn visit_update_stmt(
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

        // cursor -> update_clause
        ensure_kind(cursor, "UPDATE")?;

        let mut update_clause = create_clause(cursor, src, "UPDATE")?;
        cursor.goto_next_sibling();
        self.consume_or_complement_sql_id(cursor, src, &mut update_clause);
        self.consume_comment_in_clause(cursor, src, &mut update_clause)?;

        // 規則上でここに現れるノードは_aliasable_identifierだが、'_'から始まっているためノードに現れない。
        // _expression、_aliasable_expressionもノードに現れないため、
        // _aliasable_identifierは実質的に_aliasable_expressionと同じCSTになっている
        // update句のエイリアスはASを省略するため
        //
        // UPDATE句は基本的にテーブルエイリアスは書けないため、AS、エイリアス共に補完しない
        let table_name = self.visit_aliasable_expr(cursor, src, None)?;

        // update句を追加する
        let mut sep_lines = SeparatedLines::new();
        sep_lines.add_expr(table_name, None, vec![]);
        update_clause.set_body(Body::SepLines(sep_lines));
        statement.add_clause(update_clause);

        cursor.goto_next_sibling();

        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // set句を処理する
        ensure_kind(cursor, "set_clause")?;
        let set_clause = self.visit_set_clause(cursor, src)?;
        statement.add_clause(set_clause);

        // from句、where句、returning句を持つ可能性がある
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "from_clause" => {
                    let clause = self.visit_from_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                "join_clause" => {
                    let clause = self.visit_join_clause(cursor, src)?;
                    statement.add_clauses(clause);
                }
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
                        "visit_update_stmt(): unimplemented update_statement\n{}",
                        create_error_info(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "update_statement")?;

        Ok(statement)
    }
}
