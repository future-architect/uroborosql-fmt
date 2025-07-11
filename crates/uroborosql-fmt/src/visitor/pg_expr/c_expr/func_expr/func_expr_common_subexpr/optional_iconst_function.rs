use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        Expr, FunctionCall, FunctionCallArgs, FunctionCallKind, Location, PrimaryExpr,
        PrimaryExprKind,
    },
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{pg_ensure_kind, Visitor},
};

impl Visitor {
    /// キーワードのみ、またはキーワード + '(' + Iconst + ')' の構造の関数をフォーマットする
    pub(crate) fn handle_optional_iconst_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        keyword_kind: SyntaxKind,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // cursor -> keyword
        pg_ensure_kind!(cursor, expr: keyword_kind, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        // キーワードのみの場合
        if !cursor.goto_next_sibling() {
            let keyword = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
            return Ok(Expr::Primary(Box::new(keyword)));
        }

        // '(' があるパターン
        // '(' Iconst ')'
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut arg_loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> Iconst
        pg_ensure_kind!(cursor, SyntaxKind::Iconst, src);
        let iconst = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
        let aligned_iconst = Expr::Primary(Box::new(iconst)).to_aligned();

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        arg_loc.append(Location::from(cursor.node().range()));

        assert!(!cursor.goto_next_sibling());

        let args = FunctionCallArgs::new(vec![aligned_iconst], arg_loc);
        let function = FunctionCall::new(
            keyword_text,
            args,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_optional_iconst_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        Ok(Expr::FunctionCall(Box::new(function)))
    }
}
