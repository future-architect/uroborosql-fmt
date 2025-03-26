use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Expr},
    error::UroboroSQLFmtError,
    new_visitor::pg_error_annotation_from_cursor,
    util::{convert_keyword_case, single_space},
};

use super::Visitor;

impl Visitor {
    /// ALL, SOME, ANY の式をフォーマットする
    /// 呼出時、 cursor は subquery_Op を指し、引数には a_expr を受け取る
    /// 呼出後、 cursor は 最後の要素を指す
    pub fn handle_all_some_any_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr subquery_Op sub_type select_with_parens
        // a_expr subquery_Op sub_type '(' a_expr ')'
        // ^      ^                                ^
        // |      |                                |
        // |      └ 呼出時                          └ 呼出し後
        // └ lhs

        // cursor -> subquery_Op
        //
        // TODO: 子要素までハンドリングする
        // subquery_Op
        // - all_Op
        // - NOT_LA? (LIKE | ILIKE)
        // - OPERATOR '(' any_operator ')'
        let op = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> sub_type (ALL | ANY | SOME)
        let all_some_any_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();

        let rhs = match cursor.node().kind() {
            SyntaxKind::select_with_parens => self.visit_select_with_parens(cursor, src)?,
            SyntaxKind::LParen => {
                // 括弧で囲まれた式が来るパターン
                // a_expr subquery_Op sub_type '(' a_expr ')'
                //                             ^^^
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_all_some_any_nodes(): parenthesized expression is not implemented. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_all_some_any_nodes(): Unexpected syntax. node: {}\n{}",
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        let mut all_some_any_sub = AlignedExpr::new(lhs);

        let space = single_space();
        all_some_any_sub.add_rhs(Some(format!("{op}{space}{all_some_any_keyword}")), rhs);

        assert!(
            !cursor.goto_next_sibling(),
            "handle_all_some_any_nodes(): cursor is not at the end of the node\n{}",
            pg_error_annotation_from_cursor(cursor, src)
        );

        Ok(all_some_any_sub)
    }
}
