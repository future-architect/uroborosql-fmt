use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{create_clause, ensure_kind, Visitor},
};

impl Visitor {
    /// FOR UPDATE句をVec<Clause>で返す
    /// SELECT文で使用する
    pub(crate) fn visit_for_update_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        let mut clauses = vec![];

        // `FOR UPDATE [ OF table_name [, ...] ] [ NOWAIT ]`

        cursor.goto_first_child();

        let mut for_update_clause = create_clause(cursor, src, "FOR_UPDATE")?;

        cursor.goto_next_sibling();

        if cursor.node().kind() == "OF" {
            // `FOR UPDATE OF table_name [, ...]`

            // for_update_clauseのキーワードにOFを追加
            for_update_clause.extend_kw(cursor.node(), src);

            cursor.goto_next_sibling();

            self.consume_comment_in_clause(cursor, src, &mut for_update_clause)?;

            let table_name = self.visit_comma_sep_identifier(cursor, src)?;

            for_update_clause.set_body(table_name);
        }

        clauses.push(for_update_clause);

        if cursor.node().kind() == "NOWAIT" {
            let nowait_clause = create_clause(cursor, src, "NOWAIT")?;
            clauses.push(nowait_clause)
        }

        cursor.goto_parent();
        ensure_kind(cursor, "for_update_clause", src)?;

        Ok(clauses)
    }
}
