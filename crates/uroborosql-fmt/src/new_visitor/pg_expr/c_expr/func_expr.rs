mod func_application;
mod func_expr_common_subexpr;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Expr},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
    util::convert_keyword_case,
};

use super::Visitor;

// func_expr:
// - func_application within_group_clause? filter_clause? over_clause?
// - func_expr_common_subexpr
// - json_aggregate_func filter_clause over_clause

// within_group_clause:
// - WITHIN GROUP_P '(' sort_clause ')'

// filter_clause:
// - FILTER '(' WHERE a_expr ')'

// over_clause:
// - OVER window_specification
// - OVER ColId

// window_specification:
// - '(' opt_existing_window_name opt_partition_clause opt_sort_clause opt_frame_clause ')'

impl Visitor {
    pub fn visit_func_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // func_expr
        // - func_application within_group_clause filter_clause over_clause
        // - func_expr_common_subexpr
        // - json_aggregate_func filter_clause over_clause

        cursor.goto_first_child();
        // cursor -> func_application | func_expr_common_subexpr | json_aggregate_func

        let mut func = match cursor.node().kind() {
            SyntaxKind::func_application => self.visit_func_application(cursor, src)?,
            SyntaxKind::func_expr_common_subexpr => {
                self.visit_func_expr_common_subexpr(cursor, src)?
            }
            SyntaxKind::json_aggregate_func => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr(): json_aggregate_func is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_next_sibling();
        // cursor ->  within_group_clause?
        if cursor.node().kind() == SyntaxKind::within_group_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): within_group_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> filter_clause?
        if cursor.node().kind() == SyntaxKind::filter_clause {
            let (filter_keyword, filter_clause) = self.visit_filter_clause(cursor, src)?;
            func.set_filter_keyword(&filter_keyword);
            func.set_filter_clause(filter_clause);
        }

        // cursor -> over_clause?
        if cursor.node().kind() == SyntaxKind::over_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): over_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();

        pg_ensure_kind(cursor, SyntaxKind::func_expr, src)?;

        Ok(Expr::FunctionCall(Box::new(func)))
    }

    fn visit_filter_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(String, Clause), UroboroSQLFmtError> {
        // filter_clause:
        // - FILTER '(' WHERE a_expr ')'

        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::FILTER, src)?;
        let filter_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;

        cursor.goto_next_sibling();
        let mut where_clause = pg_create_clause(cursor, SyntaxKind::WHERE)?;

        cursor.goto_next_sibling();
        // cursor -> comment?
        self.pg_consume_comments_in_clause(cursor, &mut where_clause)?;

        // cursor -> a_expr
        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        where_clause.set_body(Body::from(expr));

        cursor.goto_next_sibling();
        // cursor -> comment?
        self.pg_consume_comments_in_clause(cursor, &mut where_clause)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;

        cursor.goto_parent();
        // cursor -> filter_clause
        pg_ensure_kind(cursor, SyntaxKind::filter_clause, src)?;

        Ok((filter_keyword, where_clause))
    }
}
