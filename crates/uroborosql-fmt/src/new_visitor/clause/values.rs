use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{values::ValuesBody, Body, Clause, Location},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
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
    ) -> Result<Clause, UroboroSQLFmtError> {
        // values_clause:
        // - VALUES '(' expr_list ')' ( ',' '(' expr_list ')' )*

        cursor.goto_first_child();

        // cursor -> VALUES
        let mut values_clause = pg_create_clause!(cursor, SyntaxKind::VALUES);

        self.pg_consume_comments_in_clause(cursor, &mut values_clause)?;

        cursor.goto_next_sibling();
        // cursor -> '('
        // values キーワード後、最初の開き括弧から後が Body の Location にあたる
        let mut body_loc = Location::from(cursor.node().range());

        let first_row = self.visit_parenthesized_expr_list(cursor, src)?;
        let mut rows = vec![first_row];

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::LParen => {
                    let parenthesized_expr_list =
                        self.visit_parenthesized_expr_list(cursor, src)?;

                    rows.push(parenthesized_expr_list);
                    // 閉じ括弧の位置で Body の Location を更新
                    body_loc.append(Location::from(cursor.node().range()));
                }
                SyntaxKind::Comma => {}
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    // TODO: コメント処理
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
            rows.iter_mut()
                .for_each(|col_list| col_list.set_force_multi_line(true));
        }

        let body = ValuesBody::new(body_loc, rows);
        values_clause.set_body(Body::Values(Box::new(body)));

        cursor.goto_parent();
        // cursor -> values_clause
        pg_ensure_kind!(cursor, SyntaxKind::values_clause, src);

        Ok(values_clause)
    }
}
