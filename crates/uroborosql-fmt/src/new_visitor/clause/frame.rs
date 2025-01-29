use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor},
};

impl Visitor {
    /// frame_clause
    pub(crate) fn visit_frame_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "frame_kind", src)?;
        cursor.goto_first_child();

        // RANGE | ROWS | GROUPS
        let mut clause = create_clause(cursor, src, cursor.node().kind())?;

        cursor.goto_parent();

        // frame_clause の各要素を Expr の Vec として持つ
        let mut exprs: Vec<Expr> = vec![];

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "BETWEEN" | "AND" => {
                    let prim = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                    exprs.push(Expr::Primary(Box::new(prim)));
                }
                "frame_bound" => {
                    exprs.extend(self.visit_frame_bound(cursor, src)?);
                }
                "frame_exclusion" => {
                    cursor.goto_first_child();

                    loop {
                        if !matches!(
                            cursor.node().kind(),
                            "EXCLUDE_CULLENT_ROW"
                                | "EXCLUDE_GROUP"
                                | "EXCLUDE_TIES"
                                | "EXCLUDE_NO_OTHERS"
                        ) {
                            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                    "visit_frame_clause(): expected EXCLUDE_{{CULLENT_ROW | GROUP | TIES | NO_OTHERS}}, but actual {}\n{}",
                                    cursor.node().kind(),
                                    error_annotation_from_cursor(cursor, src)
                                )));
                        }
                        let prim =
                            PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                        exprs.push(Expr::Primary(Box::new(prim)));

                        if !cursor.goto_next_sibling() {
                            break;
                        }
                    }
                    cursor.goto_parent();
                    ensure_kind(cursor, "frame_exclusion", src)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_frame_clause(): unexpected node\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )))
                }
            }
        }

        let n_expr = ExprSeq::new(&exprs);

        // 単一行に描画するため、SingleLineを生成する
        clause.set_body(Body::to_single_line(Expr::ExprSeq(Box::new(n_expr))));

        cursor.goto_parent();
        ensure_kind(cursor, "frame_clause", src)?;

        Ok(clause)
    }

    /// frame_clause の frame_bound 部分のフォーマット処理を行う。
    fn visit_frame_bound(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Expr>, UroboroSQLFmtError> {
        let mut exprs = vec![];
        cursor.goto_first_child();
        match cursor.node().kind() {
            "UNBOUNDED_PRECEDING" | "CURRENT_ROW" | "UNBOUNDED_FOLLOWING" => {
                let prim = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                exprs.push(Expr::Primary(Box::new(prim)));
                cursor.goto_next_sibling();

                let prim = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                exprs.push(Expr::Primary(Box::new(prim)));
                cursor.goto_next_sibling();
            }
            _ => {
                let expr = self.visit_expr(cursor, src)?;
                exprs.push(expr);
                cursor.goto_next_sibling();

                if !matches!(cursor.node().kind(), "PRECEDING" | "FOLLOWING") {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        r##"visit_frame_clause(): expect "PRECEDING" or "FOLLOWING", but actual {}\n{}"##,
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }

                let prim = PrimaryExpr::with_node(cursor.node(), src, PrimaryExprKind::Keyword);
                exprs.push(Expr::Primary(Box::new(prim)));
            }
        }
        cursor.goto_parent();
        ensure_kind(cursor, "frame_bound", src)?;

        Ok(exprs)
    }
}
