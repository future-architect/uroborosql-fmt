use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::Clause,
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, Visitor},
};

// returning_clause
// - RETURNING target_list
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
        // - RETURNING target_list

        cursor.goto_first_child();

        let mut clause = create_clause!(cursor, SyntaxKind::RETURNING);

        cursor.goto_next_sibling();
        self.consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> target_list
        ensure_kind!(cursor, SyntaxKind::target_list, src);
        let body = self.visit_target_list(cursor, src, None)?;
        clause.set_body(body);

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::returning_clause, src);

        Ok(clause)
    }
}
