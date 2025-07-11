use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Comment, Expr, PrimaryExpr, PrimaryExprKind},
    util::convert_keyword_case,
    visitor::{
        pg_ensure_kind, pg_error_annotation_from_cursor, FunctionCall, FunctionCallArgs,
        FunctionCallKind, UroboroSQLFmtError,
    },
};

use super::Visitor;

impl Visitor {
    /// keyword '(' a_expr AS typename ')' という構造の関数をフォーマットする
    /// 呼出時、cursor はキーワード（CAST or TREAT）を指している
    /// 呼出後、cursor は 最後の要素の RParen を指している
    pub(crate) fn handle_expr_as_typename_function(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        keyword_kind: SyntaxKind,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // CAST '(' a_expr AS typename ')'
        // TREAT '(' a_expr AS typename ')'

        // cursor -> keyword (CAST or TREAT)
        pg_ensure_kind!(cursor, expr: keyword_kind, src);
        let keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();
        // cursor -> comment?
        let first_arg_bind_param = if cursor.node().kind() == SyntaxKind::C_COMMENT {
            let comment = Comment::pg_new(cursor.node());
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        // cursor -> a_expr
        pg_ensure_kind!(cursor, SyntaxKind::a_expr, src);
        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> AS
        pg_ensure_kind!(cursor, SyntaxKind::AS, src);
        let as_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> Typename
        pg_ensure_kind!(cursor, SyntaxKind::Typename, src);
        let type_name = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        // 最後の要素
        assert!(!cursor.goto_next_sibling());

        let mut aligned = AlignedExpr::new(expr);
        aligned.add_rhs(Some(as_keyword), Expr::Primary(Box::new(type_name)));

        if let Some(bind_param) = first_arg_bind_param {
            if bind_param.is_block_comment() && bind_param.loc().is_next_to(&aligned.loc()) {
                aligned.set_head_comment(bind_param);
            } else {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_expr_as_typename_function(): Comments are not supported at this position\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        let args = FunctionCallArgs::new(vec![aligned], cursor.node().range().into());

        let function = FunctionCall::new(
            keyword,
            args,
            FunctionCallKind::BuiltIn,
            cursor
                .node()
                .parent()
                .expect("handle_expr_as_typename_function: cursor.node().parent() is None")
                .range()
                .into(),
        );

        Ok(function)
    }
}
