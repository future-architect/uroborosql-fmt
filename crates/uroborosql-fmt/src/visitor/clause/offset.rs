use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, SingleLine},
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor},
};

use super::Visitor;

// offset_clause:
// - OFFSET select_offset_value
// - OFFSET select_fetch_first_value row_or_rows

// select_offset_value:
// - a_expr

impl Visitor {
    pub(crate) fn visit_offset_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // offset_clause:
        // - OFFSET select_offset_value
        // - OFFSET select_fetch_first_value row_or_rows

        cursor.goto_first_child();
        ensure_kind!(cursor, SyntaxKind::OFFSET, src);

        let mut offset_clause = Clause::from_node(cursor.node());

        cursor.goto_next_sibling();

        self.consume_comments_in_clause(cursor, &mut offset_clause)?;

        // cursor -> select_offset_value | select_fetch_first_value
        match cursor.node().kind() {
            SyntaxKind::select_offset_value => {
                // select_offset_value
                // - a_expr

                cursor.goto_first_child();

                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
                let body = Body::SingleLine(Box::new(SingleLine::new(expr)));

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::select_offset_value, src);

                offset_clause.set_body(body);
            }
            SyntaxKind::select_fetch_first_value => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_offset_clause(): select_fetch_first_value is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_offset_clause(): unexpected node kind: {}\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::offset_clause, src);

        Ok(offset_clause)
    }
}
