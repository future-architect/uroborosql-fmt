use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::ColumnList,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
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

        let first_row = ColumnList::try_from(self.handle_parenthesized_expr_list(cursor, src)?)?;
        let mut rows = vec![first_row];

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::LParen => {
                    let parenthesized_expr_list =
                        ColumnList::try_from(self.handle_parenthesized_expr_list(cursor, src)?)?;

                    rows.push(parenthesized_expr_list);
                }
                SyntaxKind::Comma => {}
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_values_clause(): unexpected comment node appeared.\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_values_clause(): unexpected node kind\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // カラムリストが一つのみであるとき、複数行で描画する
        if rows.len() == 1 {
            rows.iter_mut().for_each(|row| {
                row.force_multi_line();
            });
        }

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        // cursor -> values_clause
        ensure_kind!(cursor, SyntaxKind::values_clause, src);

        Ok((convert_keyword_case(values_keyword), rows))
    }
}
