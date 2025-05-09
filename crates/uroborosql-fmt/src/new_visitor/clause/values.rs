use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::ColumnList,
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
    util::convert_keyword_case,
    NewVisitor as Visitor,
};

// values_clause:
// - VALUES '(' expr_list ')' ( ',' '(' expr_list ')' )*
//
// values_clause はフラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/22

impl Visitor {
    pub(crate) fn visit_values_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<(String, Vec<ColumnList>), UroboroSQLFmtError> {
        // values_clause:
        // - VALUES '(' expr_list ')' ( ',' '(' expr_list ')' )*

        cursor.goto_first_child();

        // cursor -> VALUES
        let values_keyword = cursor.node().text();

        cursor.goto_next_sibling();
        // cursor -> '('

        let first_row = self.visit_parenthesized_expr_list(cursor, src)?;
        let mut rows = vec![first_row];

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::LParen => {
                    let parenthesized_expr_list =
                        self.visit_parenthesized_expr_list(cursor, src)?;

                    rows.push(parenthesized_expr_list);
                }
                SyntaxKind::Comma => {}
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_values_clause(): unexpected comment node appeared.\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_values_clause(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // カラムリストが一つのみであるとき、複数行で描画する
        if rows.len() == 1 {
            rows.iter_mut().for_each(|row| {
                row.set_force_multi_line(true);
            });
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        // cursor -> values_clause
        pg_ensure_kind!(cursor, SyntaxKind::values_clause, src);

        Ok((convert_keyword_case(values_keyword), rows))
    }
}
