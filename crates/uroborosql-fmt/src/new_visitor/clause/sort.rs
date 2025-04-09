use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AlignedExpr, Body, Clause, Comment, Expr, Location, PrimaryExpr, SeparatedLines},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, COMMA},
    util::convert_keyword_case,
    NewVisitor as Visitor,
};

// sort_clause
// - ORDER BY sortby_list

// sortby_list
// - sortby ( ',' sortby )*
// flattened: https://github.com/future-architect/postgresql-cst-parser/pull/13

// sortby
// - a_expr USING qual_all_Op opt_nulls_order
// - a_expr opt_asc_desc opt_nulls_order

impl Visitor {
    pub(crate) fn visit_sort_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // sort_clause
        // - ORDER BY sortby_list

        cursor.goto_first_child();
        // cursor -> ORDER
        let mut clause = pg_create_clause!(cursor, SyntaxKind::ORDER);
        cursor.goto_next_sibling();

        // curosr -> BY
        clause.pg_extend_kw(cursor.node());
        cursor.goto_next_sibling();

        // cursor -> sortby_list
        let sortby_list = self.visit_sortby_list(cursor, src)?;
        clause.set_body(Body::SepLines(sortby_list));

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::sort_clause, src);

        Ok(clause)
    }

    fn visit_sortby_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SeparatedLines, UroboroSQLFmtError> {
        // sortby_list
        // - sortby ( ',' sortby )*

        cursor.goto_first_child();
        let mut sep_lines = SeparatedLines::new();

        // cursor -> sortby
        let first_element = self.visit_sortby(cursor, src)?;
        sep_lines.add_expr(first_element, None, vec![]);

        let mut is_preceding_comment_area = false;
        let mut preceding_comments = vec![];

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::sortby => {
                    sep_lines.add_expr(
                        self.visit_sortby(cursor, src)?,
                        Some(COMMA.to_string()),
                        preceding_comments.clone(),
                    );
                    preceding_comments.clear();
                    is_preceding_comment_area = false;
                }
                SyntaxKind::Comma => is_preceding_comment_area = true,
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    if is_preceding_comment_area {
                        preceding_comments.push(comment);
                    } else {
                        sep_lines.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_sortby_list(): unexpected node\nnode kind: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )))
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::sortby_list, src);

        Ok(sep_lines)
    }

    /// ORDER BY句の本体に現れる式を AlignedExpr で返す
    /// AlignedExpr の左辺にカラム名(式)、右辺にオプション (ASC, DESC, NULLS FIRST...)を持ち、演算子は常に空にする
    fn visit_sortby(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // sortby
        // - a_expr opt_asc_desc? opt_nulls_order?
        // - a_expr USING qual_all_Op opt_nulls_order?

        cursor.goto_first_child();

        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        let mut aligned_expr = AlignedExpr::new(expr);

        cursor.goto_next_sibling();

        // 式以降のオプション部分の要素を順番に収集する
        let mut order_option = vec![];
        let mut order_option_loc = vec![];

        // cursor -> (opt_asc_desc | USING)?
        if cursor.node().kind() == SyntaxKind::opt_asc_desc {
            // opt_asc_desc
            // - ASC | DESC

            let asc_or_desc = cursor.node().text();
            order_option.push(asc_or_desc);
            order_option_loc.push(Location::from(cursor.node().range()));

            cursor.goto_next_sibling();
        } else if cursor.node().kind() == SyntaxKind::USING {
            // USING qual_all_Op

            // USING
            let using = cursor.node().text();
            order_option.push(using);
            order_option_loc.push(Location::from(cursor.node().range()));

            cursor.goto_next_sibling();

            // cursor -> qual_all_Op
            let op = cursor.node().text();
            order_option.push(op);
            order_option_loc.push(Location::from(cursor.node().range()));

            cursor.goto_next_sibling();
        }

        // cursor -> opt_nulls_order?
        if cursor.node().kind() == SyntaxKind::opt_nulls_order {
            // opt_nulls_order
            // - NULLS_LA (FIRST_P | LAST_P)

            cursor.goto_first_child();

            // cursor -> NULLS_LA
            let nulls = cursor.node().text();
            order_option.push(nulls);
            order_option_loc.push(Location::from(cursor.node().range()));

            cursor.goto_next_sibling();

            // cursor -> FIRST_P | LAST_P
            let first_or_last = cursor.node().text();
            order_option.push(first_or_last);
            order_option_loc.push(Location::from(cursor.node().range()));

            cursor.goto_parent();
            pg_ensure_kind!(cursor, SyntaxKind::opt_nulls_order, src);
        }

        // 要素をスペース区切りで PrimaryExpr に変換し、 AlignedExpr の右辺に追加
        if !order_option.is_empty() {
            let mut loc = order_option_loc[0].clone();
            order_option_loc.into_iter().for_each(|l| loc.append(l));

            let order = PrimaryExpr::new(convert_keyword_case(&order_option.join(" ")), loc);
            aligned_expr.add_rhs(None, Expr::Primary(Box::new(order)));
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::sortby, src);

        Ok(aligned_expr)
    }
}
