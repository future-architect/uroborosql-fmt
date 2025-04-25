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
            let as_keyword = cursor.node().text().to_string();
            cursor.goto_next_sibling();

            Some(as_keyword)
        } else {
            None
        };

        let mut aligned = relation_expr.to_aligned();
        // cursor -> ColId?
        if cursor.node().kind() == SyntaxKind::ColId {
            let col_id = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
            let rhs = Expr::Primary(Box::new(col_id));

            // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
            if let Some(as_keyword) = as_keyword {
                if CONFIG.read().unwrap().remove_table_as_keyword {
                    aligned.add_rhs(None, rhs);
                } else {
                    aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), rhs);
                }
            } else {
                // AS がない場合はそのまま追加
                aligned.add_rhs(None, rhs);
            }
        };

        cursor.goto_parent();
        // cursor -> relation_expr_opt_alias
        pg_ensure_kind!(cursor, SyntaxKind::relation_expr_opt_alias, src);

        let mut body = SeparatedLines::new();
        body.add_expr(aligned, None, vec![]);

        Ok(Body::SepLines(body))
    }
}
