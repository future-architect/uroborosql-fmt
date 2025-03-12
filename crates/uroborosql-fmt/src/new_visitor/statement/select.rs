use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// SELECT文
    /// 呼び出し後、cursorはselect_statementを指す
    pub(crate) fn visit_select_stmt(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // SELECT文の定義
        // SelectStmt
        // ├── SELECT [ALL | DISTINCT] (target_list)
        // │   ├── into_clause
        // │   ├── from_clause
        // │   ├── where_clause
        // │   ├── group_clause
        // │   ├── having_clause
        // │   └── window_clause
        // ├── values_clause
        // ├── TABLE relation_expr
        // ├── (select_clause) UNION [ALL | DISTINCT] (select_clause)
        // ├── (select_clause) INTERSECT [ALL | DISTINCT] (select_clause)
        // ├── (select_clause) EXCEPT [ALL | DISTINCT] (select_clause)
        // ├── (select_clause) sort_clause
        // ├── (select_clause) opt_sort_clause for_locking_clause opt_select_limit
        // ├── (select_clause) opt_sort_clause select_limit opt_for_locking_clause
        // ├── with_clause (select_clause)
        // ├── with_clause (select_clause) sort_clause
        // ├── with_clause (select_clause) opt_sort_clause for_locking_clause opt_select_limit
        // ├── with_clause (select_clause) opt_sort_clause select_limit opt_for_locking_clause
        // └── select_with_parens
        //     ├── '(' select_no_parens ')'
        //     └── '(' select_with_parens ')'
        //
        // select_clause (clause 自体はない)
        // - context: https://github.com/future-architect/postgresql-cst-parser/pull/2#discussion_r1897026688

        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cusor -> with_clause?

        if cursor.node().kind() == SyntaxKind::with_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_select_stmt(): with_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));

            // // with句を追加する
            // let mut with_clause = self.visit_with_clause(cursor, src)?;
            // cursor.goto_next_sibling();
            // // with句の後に続くコメントを消費する
            // self.consume_comment_in_clause(cursor, src, &mut with_clause)?;

            // statement.add_clause(with_clause);
        }

        // cursor -> SELECT keyword
        // select_clause を消去したので、select_clause の中身が並ぶ
        pg_ensure_kind(cursor, SyntaxKind::SELECT, src)?;

        // select句を追加する
        statement.add_clause(self.visit_select_clause(cursor, src)?);

        while cursor.goto_next_sibling() {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            match cursor.node().kind() {
                // 現時点で考慮している構造
                // - into_clause
                // - from_clause
                // - where_clause
                // - group_clause
                // - having_clause
                // - window_clause
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                SyntaxKind::into_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): into_clause is not implemented\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::from_clause => {
                    let from_clause = self.pg_visit_from_clause(cursor, src)?;
                    statement.add_clause(from_clause);
                }
                SyntaxKind::where_clause => {
                    let where_clause = self.pg_visit_where_clause(cursor, src)?;
                    statement.add_clause(where_clause);
                }
                SyntaxKind::group_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): group_clause is not implemented\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::having_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): having_clause is not implemented\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::window_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): window_clause is not implemented\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): {} node appeared. This node is not considered yet.\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::SelectStmt, src)?;

        Ok(statement)
    }
}
