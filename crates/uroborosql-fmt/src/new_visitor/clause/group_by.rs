use std::vec;

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT,
    },
};

impl Visitor {
    /// GROUP BY句に対応するClauseを持つVecを返す。
    /// HAVING句がある場合は、HAVING句に対応するClauseも含む。
    pub(crate) fn visit_group_by_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        let mut clauses: Vec<Clause> = vec![];

        cursor.goto_first_child();

        let mut clause = create_clause(cursor, src, "GROUP_BY")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let mut sep_lines = SeparatedLines::new();
        let first = self.visit_group_expression(cursor, src)?;
        sep_lines.add_expr(first.to_aligned(), None, vec![]);

        // commaSep(group_expression)
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                COMMA => {
                    continue;
                }
                "group_expression" => {
                    let expr = self.visit_group_expression(cursor, src)?;
                    sep_lines.add_expr(expr.to_aligned(), Some(COMMA.to_string()), vec![]);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                "ERROR" => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_group_by_clause: ERROR node appeared \n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    break;
                }
            }
        }

        clause.set_body(Body::SepLines(sep_lines));
        clauses.push(clause);

        if cursor.node().kind() == "having_clause" {
            clauses.push(self.visit_having_clause(cursor, src)?);
        }

        cursor.goto_parent();
        ensure_kind(cursor, "group_by_clause", src)?;

        Ok(clauses)
    }

    fn visit_group_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let ret_value = match cursor.node().kind() {
            "grouping_sets_clause" | "rollup_clause" | "cube_clause" => {
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_group_expression(): unimplemented node\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => self.visit_expr(cursor, src),
        };

        cursor.goto_parent();
        ensure_kind(cursor, "group_expression", src)?;

        ret_value
    }
}
