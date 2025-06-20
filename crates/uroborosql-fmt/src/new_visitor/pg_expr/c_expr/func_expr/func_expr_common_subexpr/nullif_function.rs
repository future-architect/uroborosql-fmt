use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{FunctionCall, FunctionCallArgs, FunctionCallKind, Location},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, Visitor},
    util::convert_keyword_case,
};

impl Visitor {
    /// NULLIF 関数をフォーマットする
    pub(crate) fn handle_nullif_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // cursor -> NULLIF
        pg_ensure_kind!(cursor, SyntaxKind::NULLIF, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut arg_loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> a_expr (1st argument)
        pg_ensure_kind!(cursor, SyntaxKind::a_expr, src);
        let first_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        let aligned_first = first_expr.to_aligned();

        cursor.goto_next_sibling();
        // cursor -> ','
        pg_ensure_kind!(cursor, SyntaxKind::Comma, src);

        cursor.goto_next_sibling();
        // cursor -> a_expr (2nd argument)
        pg_ensure_kind!(cursor, SyntaxKind::a_expr, src);
        let second_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        let aligned_second = second_expr.to_aligned();

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        arg_loc.append(Location::from(cursor.node().range()));

        assert!(!cursor.goto_next_sibling());

        let args = FunctionCallArgs::new(vec![aligned_first, aligned_second], arg_loc);
        let function = FunctionCall::new(
            keyword_text,
            args,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_nullif_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        Ok(function)
    }
}
