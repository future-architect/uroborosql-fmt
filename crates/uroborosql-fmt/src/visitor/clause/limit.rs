use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, Visitor, COMMENT},
};

impl Visitor {
    /// LIMIT句をClause構造体で返す
    /// SELECT文で使用する
    pub(crate) fn visit_limit_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();
        ensure_kind(cursor, "LIMIT")?;
        let mut limit_clause = Clause::from_node(cursor.node(), src);

        cursor.goto_next_sibling();
        // cursor -> number | ALL

        if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            cursor.goto_next_sibling();
            limit_clause.add_comment_to_child(comment)?;
        }

        match cursor.node().kind() {
            "number" => {
                // numberをExprに格納
                let number = self.visit_expr(cursor, src)?;
                // numberからBody::SingleLineを作成
                let body = Body::SingleLine(Box::new(SingleLine::new(number)));
                limit_clause.set_body(body);
            }
            "ALL" => {
                // "LIMIT ALL"というキーワードと捉えて構造体に格納
                let all_kw = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                let expr: Expr = Expr::Primary(Box::new(all_kw));
                let body = Body::SingleLine(Box::new(SingleLine::new(expr)));
                limit_clause.set_body(body);
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    r#"visit_limit_clause(): expected node is number or ALL, but actual {}\n{:#?}"#,
                    cursor.node().kind(),
                    cursor.node().range()
                )));
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "limit_clause")?;

        Ok(limit_clause)
    }
}
