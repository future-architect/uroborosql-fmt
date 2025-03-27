use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, SeparatedLines},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, COMMA},
    NewVisitor as Visitor,
};

// for_locking_clause
// - for_locking_items
// - FOR READ ONLY
//
// for_locking_items
// - for_locking_item (for_locking_item)*
//
// for_locking_item
// - for_locking_strength locked_rels_list? opt_nowait_or_skip?
//
// for_locking_strength
// - FOR UPDATE
// - FOR NO KEY UPDATE
// - FOR SHARE
// - FOR KEY SHARE
//
// locked_rels_list
// - OF qualified_name_list
//
// opt_nowait_or_skip
// - NOWAIT
// - SKIP LOCKED

impl Visitor {
    pub(crate) fn visit_for_locking_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        // for_locking_clause
        // - for_locking_items
        // - FOR READ ONLY

        cursor.goto_first_child();

        let clauses = match cursor.node().kind() {
            SyntaxKind::for_locking_items => self.visit_for_locking_items(cursor, src)?,
            SyntaxKind::FOR => {
                // FOR READ ONLY
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_for_locking_clause: FOR READ ONLY is not implemented"
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_for_locking_clause: unexpected node kind: {}",
                    cursor.node().kind()
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::for_locking_clause, src)?;

        Ok(clauses)
    }

    fn visit_for_locking_items(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        // for_locking_items
        // - for_locking_item (for_locking_item)*
        let mut clauses = vec![];

        cursor.goto_first_child();

        // first for_locking_item
        self.visit_for_locking_item(cursor, src, &mut clauses)?;

        // cursor -> for_locking_item
        while cursor.goto_next_sibling() {
            self.visit_for_locking_item(cursor, src, &mut clauses)?;
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::for_locking_items, src)?;

        Ok(clauses)
    }

    fn visit_for_locking_item(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clauses: &mut Vec<Clause>,
    ) -> Result<(), UroboroSQLFmtError> {
        // for_locking_item
        // - for_locking_strength locked_rels_list? opt_nowait_or_skip?

        cursor.goto_first_child();

        // cursor -> for_locking_strength
        let mut for_update_clause = self.visit_for_locking_strength(cursor, src)?;
        cursor.goto_next_sibling();

        // cursor -> locked_rels_list?
        if cursor.node().kind() == SyntaxKind::locked_rels_list {
            self.visit_locked_rels_list(cursor, src, &mut for_update_clause)?;
            cursor.goto_next_sibling();
        }
        clauses.push(for_update_clause);

        // cursor -> opt_nowait_or_skip?
        if cursor.node().kind() == SyntaxKind::opt_nowait_or_skip {
            let no_wait_clause = self.visit_opt_nowait_or_skip(cursor, src)?;
            clauses.push(no_wait_clause);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::for_locking_item, src)?;
        Ok(())
    }

    fn visit_for_locking_strength(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // for_locking_strength
        // - FOR UPDATE
        // - FOR NO KEY UPDATE
        // - FOR SHARE
        // - FOR KEY SHARE
        cursor.goto_first_child();

        let mut clause = Clause::from_pg_node(cursor.node());

        while cursor.goto_next_sibling() {
            clause.pg_extend_kw(cursor.node());
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::for_locking_strength, src)?;

        Ok(clause)
    }

    fn visit_locked_rels_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clause: &mut Clause,
    ) -> Result<(), UroboroSQLFmtError> {
        // locked_rels_list
        // - OF qualified_name_list

        cursor.goto_first_child();

        clause.pg_extend_kw(cursor.node());
        cursor.goto_next_sibling();

        let table_name_list = self.visit_qualified_name_list(cursor, src)?;

        clause.set_body(table_name_list);

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::locked_rels_list, src)?;

        Ok(())
    }

    fn visit_qualified_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // qualified_name_list
        // - qualified_name (',' qualified_name)*

        cursor.goto_first_child();

        let mut separated_lines = SeparatedLines::new();

        let first_element = self.visit_qualified_name(cursor, src)?;
        separated_lines.add_expr(first_element.to_aligned(), None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::qualified_name => {
                    let element = self.visit_qualified_name(cursor, src)?;
                    separated_lines.add_expr(element.to_aligned(), Some(COMMA.to_string()), vec![]);
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_qualified_name_list: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::qualified_name_list, src)?;

        Ok(Body::SepLines(separated_lines))
    }

    fn visit_opt_nowait_or_skip(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // opt_nowait_or_skip
        // - NOWAIT
        // - SKIP LOCKED

        cursor.goto_first_child();

        let mut clause = Clause::from_pg_node(cursor.node());

        if cursor.goto_next_sibling() {
            clause.pg_extend_kw(cursor.node());
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::opt_nowait_or_skip, src)?;

        Ok(clause)
    }
}
