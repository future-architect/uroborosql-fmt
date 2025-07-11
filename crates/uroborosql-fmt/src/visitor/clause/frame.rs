use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Expr, ExprSeq, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor},
};

impl Visitor {
    pub(crate) fn visit_opt_frame_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // opt_frame_clause:
        // - (RANGE | ROWS | GROUPS) frame_extent opt_window_exclusion_clause?

        cursor.goto_first_child();

        // cursor -> RANGE | ROWS | GROUPS
        let mut clause = pg_create_clause!(
            cursor,
            SyntaxKind::RANGE | SyntaxKind::ROWS | SyntaxKind::GROUPS
        );

        // frame 句の各要素を Expr の Vec として持つ
        let mut exprs: Vec<Expr> = vec![];

        cursor.goto_next_sibling();
        // cursor -> frame_extent
        let extent_exprs = self.visit_frame_extent(cursor, src)?;
        exprs.extend(extent_exprs);

        cursor.goto_next_sibling();
        // cursor -> opt_window_exclusion_clause?
        if cursor.node().kind() == SyntaxKind::opt_window_exclusion_clause {
            let exclusion_exprs = self.visit_opt_window_exclusion_clause(cursor, src)?;
            exprs.extend(exclusion_exprs);
        }

        let expr_seq = ExprSeq::new(&exprs);

        // 単一行に描画するため、SingleLineを生成する
        clause.set_body(Body::to_single_line(Expr::ExprSeq(Box::new(expr_seq))));

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_frame_clause, src);

        Ok(clause)
    }

    fn visit_frame_extent(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Expr>, UroboroSQLFmtError> {
        // frame_extent:
        // - frame_bound
        // - BETWEEN frame_bound AND frame_bound

        cursor.goto_first_child();

        let mut exprs: Vec<Expr> = vec![];

        loop {
            match cursor.node().kind() {
                SyntaxKind::frame_bound => {
                    let bound_exprs = self.visit_frame_bound(cursor, src)?;
                    exprs.extend(bound_exprs);
                }
                SyntaxKind::BETWEEN | SyntaxKind::AND => {
                    let primary =
                        PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
                    exprs.push(Expr::Primary(Box::new(primary)));
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        r##"visit_frame_extent(): expect "BETWEEN" or "AND" or "frame_bound", but actual {}\n{}"##,
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            };

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::frame_extent, src);

        Ok(exprs)
    }

    fn visit_frame_bound(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Expr>, UroboroSQLFmtError> {
        // frame_bound:
        // - UNBOUNDED (PRECEDING | FOLLOWING)
        // - CURRENT_P ROW
        // - a_expr (PRECEDING | FOLLOWING)

        cursor.goto_first_child();

        let mut exprs: Vec<Expr> = vec![];

        loop {
            let expr = match cursor.node().kind() {
                SyntaxKind::UNBOUNDED
                | SyntaxKind::PRECEDING
                | SyntaxKind::FOLLOWING
                | SyntaxKind::CURRENT_P
                | SyntaxKind::ROW => {
                    let primary =
                        PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
                    Expr::Primary(Box::new(primary))
                }
                SyntaxKind::a_expr => self.visit_a_expr_or_b_expr(cursor, src)?,
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        r##"visit_frame_bound(): expect "UNBOUNDED" or "CURRENT_P_ROW" or "a_expr" or "PRECEDING" or "FOLLOWING" or "ROW", but actual {}\n{}"##,
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            };

            exprs.push(expr);

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::frame_bound, src);

        Ok(exprs)
    }

    fn visit_opt_window_exclusion_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Expr>, UroboroSQLFmtError> {
        // opt_window_exclusion_clause:
        // - EXCLUDE CURRENT_P ROW
        // - EXCLUDE GROUP_P
        // - EXCLUDE TIES
        // - EXCLUDE NO OTHERS

        cursor.goto_first_child();

        let mut exprs: Vec<Expr> = vec![];

        loop {
            match cursor.node().kind() {
                SyntaxKind::EXCLUDE
                | SyntaxKind::CURRENT_P
                | SyntaxKind::ROW
                | SyntaxKind::GROUP_P
                | SyntaxKind::TIES
                | SyntaxKind::NO
                | SyntaxKind::OTHERS => {
                    let primary =
                        PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?;
                    exprs.push(Expr::Primary(Box::new(primary)));
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        r##"visit_opt_window_exclusion_clause(): expect "EXCLUDE" or "CURRENT_P_ROW" or "GROUP_P" or "TIES" or "NO" or "OTHERS", but actual {}\n{}"##,
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_window_exclusion_clause, src);

        Ok(exprs)
    }
}
