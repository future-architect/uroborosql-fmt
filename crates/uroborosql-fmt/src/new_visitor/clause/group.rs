use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Comment, Expr, SeparatedLines},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, COMMA},
    NewVisitor as Visitor,
};

// group_clause
// - GROUP_P BY set_quantifier group_by_list

// group_by_list
// - group_by_item (',' group_by_item)*
// flattened: https://github.com/future-architect/postgresql-cst-parser/pull/14

// group_by_item
// - a_expr
// - empty_grouping_set
// - cube_clause
// - rollup_clause
// - grouping_sets_clause

impl Visitor {
    /// GROUP BY句に対応するClauseを返す
    pub(crate) fn visit_group_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // group_clause
        // - GROUP_P BY set_quantifier group_by_list

        cursor.goto_first_child();

        // cursor -> GROUP_P
        let mut clause = pg_create_clause(cursor, SyntaxKind::GROUP_P)?;
        cursor.goto_next_sibling();

        // cursor -> BY
        clause.pg_extend_kw(cursor.node());
        cursor.goto_next_sibling();
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> set_quantifier?
        if cursor.node().kind() == SyntaxKind::set_quantifier {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_group_clause(): set_quantifier is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        let group_by_list = self.visit_group_by_list(cursor, src)?;
        clause.set_body(Body::SepLines(group_by_list));

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::group_clause, src)?;

        Ok(clause)
    }

    fn visit_group_by_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SeparatedLines, UroboroSQLFmtError> {
        // group_by_list
        // - group_by_item (',' group_by_item)*
        // flattened: https://github.com/future-architect/postgresql-cst-parser/pull/14

        cursor.goto_first_child();

        let mut sep_lines = SeparatedLines::new();

        let first = self.visit_group_by_item(cursor, src)?;
        sep_lines.add_expr(first.to_aligned(), None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {
                    continue;
                }
                SyntaxKind::group_by_item => {
                    let expr = self.visit_group_by_item(cursor, src)?;
                    sep_lines.add_expr(expr.to_aligned(), Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_group_by_list(): unexpected node\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::group_by_list, src)?;

        Ok(sep_lines)
    }

    fn visit_group_by_item(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // group_by_item
        // - a_expr
        // - empty_grouping_set
        // - cube_clause
        // - rollup_clause
        // - grouping_sets_clause

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::a_expr => self.visit_a_expr_or_b_expr(cursor, src)?,
            SyntaxKind::empty_grouping_set
            | SyntaxKind::cube_clause
            | SyntaxKind::rollup_clause
            | SyntaxKind::grouping_sets_clause => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_group_by_item(): unimplemented node\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_group_by_item(): unexpected node\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::group_by_item, src)?;

        Ok(expr)
    }
}
