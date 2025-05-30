use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{FunctionCall, FunctionCallArgs, FunctionCallKind},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, Visitor},
    util::convert_keyword_case,
};

impl Visitor {
    /// keyword '(' expr_list ')' という構造の関数をフォーマットする
    /// 呼出時、cursor はキーワードを指している
    /// 呼出後、cursor は 最後の要素の RParen を指している
    pub(crate) fn handle_expr_list_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        keyword_kind: SyntaxKind,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // keyword '(' expr_list ')'

        // cursor -> keyword
        pg_ensure_kind!(cursor, expr: keyword_kind, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();
        // cursor -> expr_list
        let expr_list = self.visit_expr_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        let args = FunctionCallArgs::new(expr_list, cursor.node().range().into());

        let function = FunctionCall::new(
            keyword_text,
            args,
            FunctionCallKind::BuiltIn,
            cursor.node().parent().expect("handle_expr_list_function: cursor.node().parent() is None").range().into(),
        );

        Ok(function)
    }
} 