use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Comment, Expr, ExprSeq, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    visitor::ensure_kind,
};

use super::Visitor;
impl Visitor {
    /// 左辺の式とオプションのNOTキーワードを受け取り、LIKE式にあたるノード群を走査する
    ///
    /// 呼出時、 cursor は LIKE_P を指している
    /// 呼出後、cursor は同階層の最後の要素 (a_expr) を指している
    ///
    pub(crate) fn handle_like_expr_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        lhs: Expr,
        not_keyword: Option<&str>,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // a_expr NOT? LIKE a_expr (ESCAPE a_expr)?
        // ^      ^         ^    ^              ^
        // lhs    |         │    │              └ 呼出後(pattern 2)
        //        |         │    └ 呼出後(pattern 1)
        //        |         └ 呼出時
        //        |
        //        └ not_keyword

        // cursor -> LIKE
        ensure_kind!(cursor, SyntaxKind::LIKE, src);

        // op_text: NOT LIKE or LIKE
        let op_text = if let Some(not_keyword) = not_keyword {
            let mut op_text = String::from(not_keyword);
            op_text.push(' ');
            op_text.push_str(cursor.node().text());
            op_text
        } else {
            cursor.node().text().to_string()
        };

        cursor.goto_next_sibling();

        // cursor -> a_expr
        let pattern = self.visit_a_expr_or_b_expr(cursor, src)?;
        cursor.goto_next_sibling();

        let mut comments_after_pattern = Vec::new();
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            comments_after_pattern.push(comment);
            cursor.goto_next_sibling();
        }

        let mut exprs = vec![pattern];

        if cursor.node().kind() == SyntaxKind::ESCAPE {
            // cursor -> (ESCAPE _expression)?
            let escape_keyword = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Keyword)?;
            let escape_keyword = Expr::Primary(Box::new(escape_keyword));

            cursor.goto_next_sibling();
            let escape_character = self.visit_a_expr_or_b_expr(cursor, src)?;

            exprs.push(escape_keyword);
            exprs.push(escape_character);
        };

        let expr_seq = Expr::ExprSeq(Box::new(ExprSeq::new(&exprs)));

        let mut aligned = AlignedExpr::new(lhs);
        aligned.add_rhs(Some(op_text), expr_seq);

        // cursor は最後の要素を指している
        assert!(!cursor.goto_next_sibling());

        Ok(aligned)
    }
}
