mod delete;
mod select;
mod update;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Expr, PrimaryExpr, PrimaryExprKind, SeparatedLines},
    error::UroboroSQLFmtError,
    new_visitor::pg_ensure_kind,
    util::convert_keyword_case,
    NewVisitor as Visitor, CONFIG,
};

impl Visitor {
    pub(crate) fn visit_relation_expr_opt_alias(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // relation_expr_opt_alias
        // - relation_expr AS? ColId?

        cursor.goto_first_child();

        let relation_expr = self.visit_relation_expr(cursor, src)?;

        cursor.goto_next_sibling();

        // cursor -> AS?
        let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            if CONFIG.read().unwrap().remove_table_as_keyword {
                // AS を除去する設定が有効ならば AS キーワードを除去する
                None
            } else {
                let as_keyword = cursor.node().text().to_string();
                cursor.goto_next_sibling();

                Some(convert_keyword_case(&as_keyword))
            }
        } else {
            None
        };

        let mut aligned = relation_expr.to_aligned();
        // cursor -> ColId?
        if cursor.node().kind() == SyntaxKind::ColId {
            let col_id = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
            let rhs = Expr::Primary(Box::new(col_id));

            aligned.add_rhs(as_keyword, rhs);
        };

        cursor.goto_parent();
        // cursor -> relation_expr_opt_alias
        pg_ensure_kind!(cursor, SyntaxKind::relation_expr_opt_alias, src);

        let mut body = SeparatedLines::new();
        body.add_expr(aligned, None, vec![]);

        Ok(Body::SepLines(body))
    }
}
