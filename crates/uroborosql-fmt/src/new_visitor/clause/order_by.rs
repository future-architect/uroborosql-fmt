use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT,
    },
    util::convert_keyword_case,
};

impl Visitor {
    pub(crate) fn visit_order_by_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        // "ORDER_BY"
        let mut clause = create_clause(cursor, src, "ORDER_BY")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let mut sep_lines = SeparatedLines::new();

        let first = self.visit_order_expression(cursor, src)?;
        sep_lines.add_expr(first, None, vec![]);

        let mut is_preceding_comment_area = false;
        let mut preceding_comments = vec![];
        // commaSep(order_expression)
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "order_expression" => {
                    sep_lines.add_expr(
                        self.visit_order_expression(cursor, src)?,
                        Some(COMMA.to_string()),
                        preceding_comments.clone(),
                    );
                    preceding_comments.clear();
                    is_preceding_comment_area = false;
                }

                COMMA => is_preceding_comment_area = true,
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    if is_preceding_comment_area {
                        preceding_comments.push(comment);
                    } else {
                        sep_lines.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_order_by_clause(): unexpected node\nnode kind: {}\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )))
                }
            }
        }

        let body = Body::SepLines(sep_lines);
        clause.set_body(body);

        cursor.goto_parent();
        ensure_kind(cursor, "order_by_clause", src)?;

        Ok(clause)
    }

    /// ORDER BY句の本体に現れる式を AlignedExpr で返す
    /// AlignedExpr の左辺にカラム名(式)、右辺にオプション (ASC, DESC, NULLS FIRST...)を持ち、演算子は常に空にする
    fn visit_order_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let expr = self.visit_expr(cursor, src)?;

        cursor.goto_next_sibling();

        let order_expr = self.visit_order_option(cursor, src, expr)?;

        cursor.goto_parent();
        ensure_kind(cursor, "order_expression", src)?;

        Ok(order_expr)
    }

    /// order_expression のオプション部分を担当する
    /// 引数に受け取った expr を左辺とする AlignedExpr を返す
    fn visit_order_option(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        expr: Expr,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        let mut order_expr = AlignedExpr::new(expr);

        // オプション
        let mut order = vec![];
        // オプションの Location
        let mut order_loc = vec![];

        // ASC | DESC
        if matches!(cursor.node().kind(), "ASC" | "DESC") {
            let asc_or_desc = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(asc_or_desc);
            order_loc.push(Location::new(cursor.node().range()));

            cursor.goto_next_sibling();
        }

        // NULLS FIRST | NULLS LAST
        if matches!(cursor.node().kind(), "NULLS") {
            let nulls = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(nulls);
            order_loc.push(Location::new(cursor.node().range()));
            cursor.goto_next_sibling();

            let first_or_last = cursor.node().utf8_text(src.as_bytes()).unwrap();
            order.push(first_or_last);
            order_loc.push(Location::new(cursor.node().range()));
            cursor.goto_next_sibling();
        };

        if !order.is_empty() {
            // Location を計算
            let mut loc = order_loc[0].clone();
            order_loc.into_iter().for_each(|l| loc.append(l));

            let order = PrimaryExpr::new(convert_keyword_case(&order.join(" ")), loc);
            order_expr.add_rhs(None, Expr::Primary(Box::new(order)));
        }

        Ok(order_expr)
    }
}
