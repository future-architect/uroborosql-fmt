use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Comment, Expr, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor},
};

use super::super::Visitor;

impl Visitor {
    /// 左辺の式を受け取り、COLLATE にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は COLLATE を指している
    /// 呼出後、 cursor は any_name を指している
    ///
    pub fn handle_collate_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr COLLATE any_name
        // ^      ^       ^
        // lhs    │       └ 呼出後
        //        └ 呼出時

        // cursor -> COLLATE
        ensure_kind!(cursor, SyntaxKind::COLLATE, src);
        let collate = convert_keyword_case(cursor.node().text());
        cursor.goto_next_sibling();

        // cursor -> comment?
        // バインドパラメータを想定
        let bind_param = if cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            cursor.goto_next_sibling();

            Some(comment)
        } else {
            None
        };

        // cursor -> any_name
        ensure_kind!(cursor, SyntaxKind::any_name, src);
        let mut any_name = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Expr)?;

        if let Some(comment) = bind_param {
            if comment.is_block_comment() && comment.loc().is_next_to(&any_name.loc()) {
                any_name.set_head_comment(comment);
            } else {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_collate_expr_nodes(): Unexpected comment\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(collate), Expr::Primary(Box::new(any_name)));

        Ok(aligned)
    }
}
