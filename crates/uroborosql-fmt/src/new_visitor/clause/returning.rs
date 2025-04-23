use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::Clause,
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind},
    NewVisitor as Visitor,
};

// returning_clause
// - RETURNING returning_with_clause? target_list
//
// returning_with_clause
// - WITH ( returning_options )
//
// returning_options
// - NEEDS_FLATTEN
// - returning_option (, returning_option )*
//
// returning_option
// - returning_option_kind AS ColId
//
// returning_option_kind
// - OLD
// - NEW

impl Visitor {
    pub(crate) fn visit_returning_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // returning_clause
        // - RETURNING returning_with_clause? target_list

        cursor.goto_first_child();

        let mut clause = pg_create_clause!(cursor, SyntaxKind::RETURNING);

        cursor.goto_next_sibling();

        // FIXME: returning_with_clause が SyntaxKind のメンバとして存在しない
        //
        // cursor -> returning_with_clause?
        // if cursor.node().kind() == SyntaxKind::returning_with_clause {
        //     return Err(UroboroSQLFmtError::Unimplemented(
        //         format!(
        //             "visit_returning_clause(): returning_with_clause is not implemented\n{}",
        //             pg_error_annotation_from_cursor(cursor, src)
        //         )
        //     ));
        // }

        // cursor -> target_list
        let body = self.visit_target_list(cursor, src)?;
        clause.set_body(body);

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::returning_clause, src);

        Ok(clause)
    }
}
