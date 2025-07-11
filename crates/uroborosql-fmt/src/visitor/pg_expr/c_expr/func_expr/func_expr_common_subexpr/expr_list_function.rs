use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{FunctionCall, FunctionCallKind},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, Visitor},
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
        ensure_kind!(cursor, expr: keyword_kind, src);
        let keyword_text = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('

        // handle_parenthesized_expr_listを使用して括弧付き式リストを処理
        let parenthesized_expr_list = self.handle_parenthesized_expr_list(cursor, src)?;

        let function = FunctionCall::new(
            keyword_text,
            parenthesized_expr_list.try_into()?,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_expr_list_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        ensure_kind!(cursor, SyntaxKind::RParen, src);

        Ok(function)
    }
}
