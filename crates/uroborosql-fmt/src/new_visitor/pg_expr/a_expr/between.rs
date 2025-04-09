use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Comment, Expr},
    error::UroboroSQLFmtError,
    new_visitor::pg_error_annotation_from_cursor,
    pg_ensure_kind,
    util::convert_keyword_case,
};

use super::Visitor;

impl Visitor {
    pub fn handle_between_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        first_expr: Expr,
        not_keyword: Option<&str>,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr NOT_LA? BETWEEN (opt_asymmetric|SYMMETRIC)? b_expr AND a_expr
        // ^      ^       ^                                             ^
        // |      |       |                                             |
        // |      |       cursor (呼出し時)                              cursor (呼出し後)
        // |      |
        // |     not_keyword
        // first_expr

        let mut operator = String::new();

        if let Some(not_keyword) = not_keyword {
            operator += &convert_keyword_case(not_keyword);
            operator += " "; // betweenの前に空白を入れる
        }

        // cursor -> BETWEEN
        pg_ensure_kind!(cursor, SyntaxKind::BETWEEN, src);
        let between_keyword = cursor.node().text();
        operator += &convert_keyword_case(between_keyword);
        cursor.goto_next_sibling();

        // cursor -> (opt_asymmetric|SYMMETRIC)?
        if cursor.node().kind() == SyntaxKind::SYMMETRIC {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "handle_between_expr_nodes(): SYMMETRIC keyword is not implemented.\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        } else if cursor.node().kind() == SyntaxKind::opt_asymmetric {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "handle_between_expr_nodes(): ASYMMETRIC keyword is not implemented.\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> b_expr
        let from_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        cursor.goto_next_sibling();

        // AND の直前に現れる行末コメントを処理する
        // 行末コメント以外のコメントは想定しない
        // TODO: 左辺に行末コメントが現れた場合のコメント縦ぞろえ
        let start_trailing_comment = if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        // cursor -> AND
        pg_ensure_kind!(cursor, SyntaxKind::AND, src);
        let and_keyword = cursor.node().text();
        cursor.goto_next_sibling();

        // cursor -> a_expr
        let to_expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        // (from_expr AND to_expr) をAlignedExprにまとめる
        let mut rhs = AlignedExpr::new(from_expr);
        rhs.add_rhs(Some(convert_keyword_case(and_keyword)), to_expr);

        if let Some(comment) = start_trailing_comment {
            rhs.set_lhs_trailing_comment(comment)?;
        }

        // (expr BETWEEN rhs) をAlignedExprにまとめる
        let mut aligned = AlignedExpr::new(first_expr);
        aligned.add_rhs(Some(operator), Expr::Aligned(Box::new(rhs)));

        // cursor -> (last node)
        assert!(
            !cursor.goto_next_sibling(),
            "handle_between_expr_nodes(): cursor is not at the last node."
        );

        Ok(aligned)
    }
}
