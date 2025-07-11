use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Comment, Expr, PrimaryExpr, PrimaryExprKind, SingleLine},
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor},
};

use super::Visitor;

// limit_clause:
// - LIMIT select_limit_value
// - LIMIT select_limit_value ',' select_offset_value # この記法は PostgreSQL ではエラーになるため考慮しない
// - FETCH first_or_next select_fetch_first_value? row_or_rows (ONLY | WITH TIES)

// select_limit_value:
// - a_expr
// - ALL

impl Visitor {
    pub(crate) fn visit_limit_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // limit_clause
        // - LIMIT select_limit_value
        // - LIMIT select_limit_value ',' select_offset_value # この記法は PostgreSQL ではエラーになるため考慮しない
        // - FETCH first_or_next select_fetch_first_value? row_or_rows (ONLY | WITH TIES)

        cursor.goto_first_child();

        // cursor -> LIMIT | FETCH
        let mut limit_clause = if cursor.node().kind() == SyntaxKind::LIMIT {
            let node = cursor.node();
            cursor.goto_next_sibling();

            Clause::from_node(node)
        } else if cursor.node().kind() == SyntaxKind::FETCH {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_limit_clause(): FETCH is not supported\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        } else {
            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_limit_clause(): unexpected node kind: {}\n{}",
                cursor.node().kind(),
                error_annotation_from_cursor(cursor, src)
            )));
        };

        // cursor -> comment?
        if cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            cursor.goto_next_sibling();
            limit_clause.add_comment_to_child(comment)?;
        }

        // cursor -> select_limit_value
        self.visit_select_limit_value(cursor, src, &mut limit_clause)?;

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::limit_clause, src);

        Ok(limit_clause)
    }

    fn visit_select_limit_value(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        limit_clause: &mut Clause,
    ) -> Result<(), UroboroSQLFmtError> {
        // select_limit_value
        // - a_expr
        // - ALL

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::ALL => {
                let all_keyword = PrimaryExpr::with_node(cursor.node(), PrimaryExprKind::Keyword)?;
                let expr = Expr::Primary(Box::new(all_keyword));
                let body = Body::SingleLine(Box::new(SingleLine::new(expr)));
                limit_clause.set_body(body);
            }
            SyntaxKind::a_expr => {
                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
                let body = Body::SingleLine(Box::new(SingleLine::new(expr)));
                limit_clause.set_body(body);
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_select_limit_value(): unexpected node kind: {}\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::select_limit_value, src);

        Ok(())
    }
}
