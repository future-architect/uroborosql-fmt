use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, Visitor},
    util::convert_keyword_case,
};

impl Visitor {
    pub(crate) fn pg_visit_where_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // cursor -> WHERE
        let mut clause = pg_create_clause!(cursor, SyntaxKind::WHERE);
        cursor.goto_next_sibling();
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        let extra_leading_boolean_operator =
            if cursor.node().kind() == SyntaxKind::AND || cursor.node().kind() == SyntaxKind::OR {
                let text = convert_keyword_case(cursor.node().text());

                cursor.goto_next_sibling();
                Some(text)
            } else {
                None
            };

        // cursor -> a_expr
        let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        if let Some(sep) = extra_leading_boolean_operator {
            if let Expr::Boolean(sep_lines) = &mut expr {
                sep_lines.set_first_separator(sep);
            } else {
                unreachable!("where_clause: Found extra boolean operator but expr is not Boolean.");
            }
        }

        // 結果として得られた式をBodyに変換する
        let body = Body::from(expr);

        clause.set_body(body);

        // cursorをwhere_clauseに戻す
        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::where_clause, src);

        Ok(clause)
    }
}
