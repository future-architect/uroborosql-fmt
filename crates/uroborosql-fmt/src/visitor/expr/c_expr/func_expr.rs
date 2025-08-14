mod func_application;
mod func_expr_common_subexpr;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Clause, Expr, Location, SeparatedLines},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor},
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
// - '(' opt_existing_window_name opt_partition_clause sort_clause? opt_frame_clause ')'

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

        // cursor -> func_expr_common_subexpr
        if cursor.node().kind() == SyntaxKind::func_expr_common_subexpr {
            let expr = self.visit_func_expr_common_subexpr(cursor, src)?;
            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::func_expr, src);
            return Ok(expr);
        }

        // cursor -> func_application | json_aggregate_func
        let mut func = match cursor.node().kind() {
            SyntaxKind::func_application => self.visit_func_application(cursor, src)?,
            SyntaxKind::json_aggregate_func => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr(): json_aggregate_func is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_expr(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_next_sibling();
        // cursor ->  within_group_clause?
        if cursor.node().kind() == SyntaxKind::within_group_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): within_group_clause is not implemented\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> filter_clause?
        if cursor.node().kind() == SyntaxKind::filter_clause {
            let (filter_keyword, filter_clause) = self.visit_filter_clause(cursor, src)?;
            func.set_filter_keyword(&filter_keyword);
            func.set_filter_clause(filter_clause);

            func.append_loc(Location::from(cursor.node().range()));
        }

        // cursor -> over_clause?
        if cursor.node().kind() == SyntaxKind::over_clause {
            let (over_keyword, over_window_definition) = self.visit_over_clause(cursor, src)?;
            func.set_over_keyword(&over_keyword);
            func.set_over_window_definition(&over_window_definition);

            func.append_loc(Location::from(cursor.node().range()));
        }

        cursor.goto_parent();

        ensure_kind!(cursor, SyntaxKind::func_expr, src);

        Ok(Expr::FunctionCall(Box::new(func)))
    }

    pub(crate) fn visit_func_expr_windowless(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // func_expr_windowless:
        // - func_application
        // - func_expr_common_subexpr
        // - json_aggregate_func

        cursor.goto_first_child();

        // cursor -> func_application | func_expr_common_subexpr | json_aggregate_func
        let expr = match cursor.node().kind() {
            SyntaxKind::func_application => {
                let func = self.visit_func_application(cursor, src)?;
                Expr::FunctionCall(Box::new(func))
            }
            SyntaxKind::func_expr_common_subexpr => {
                self.visit_func_expr_common_subexpr(cursor, src)?
            }
            SyntaxKind::json_aggregate_func => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_expr_windowless(): json_aggregate_func is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_expr_windowless(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_expr_windowless, src);

        Ok(expr)
    }

    fn visit_filter_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(String, Clause), UroboroSQLFmtError> {
        // filter_clause:
        // - FILTER '(' WHERE a_expr ')'

        cursor.goto_first_child();
        ensure_kind!(cursor, SyntaxKind::FILTER, src);
        let filter_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();
        let mut where_clause = create_clause!(cursor, SyntaxKind::WHERE);

        cursor.goto_next_sibling();
        // cursor -> comment?
        self.consume_comments_in_clause(cursor, &mut where_clause)?;

        // cursor -> a_expr
        let expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        where_clause.set_body(Body::from(expr));

        cursor.goto_next_sibling();
        // cursor -> comment?
        self.consume_comments_in_clause(cursor, &mut where_clause)?;

        cursor.goto_next_sibling();
        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        // cursor -> filter_clause
        ensure_kind!(cursor, SyntaxKind::filter_clause, src);

        Ok((filter_keyword, where_clause))
    }

    fn visit_over_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(String, Vec<Clause>), UroboroSQLFmtError> {
        // over_clause:
        // - OVER window_specification
        // - OVER ColId

        cursor.goto_first_child();
        // cursor -> OVER
        ensure_kind!(cursor, SyntaxKind::OVER, src);
        let over_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();

        // cursor -> window_specification | ColId
        let clauses = match cursor.node().kind() {
            SyntaxKind::window_specification => self.visit_window_specification(cursor, src)?,
            SyntaxKind::ColId => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_over_clause(): ColId is not implemented\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_over_clause(): unexpected node kind\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::over_clause, src);

        Ok((over_keyword, clauses))
    }

    fn visit_window_specification(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        // window_specification:
        // - '(' opt_existing_window_name? opt_partition_clause? sort_clause? opt_frame_clause? ')'

        cursor.goto_first_child();
        ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();

        let mut clauses = vec![];

        // cursor -> opt_existing_window_name?
        if cursor.node().kind() == SyntaxKind::opt_existing_window_name {
            // opt_existing_window_name:
            // - ColId
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_window_specification(): opt_existing_window_name is not implemented\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> opt_partition_clause?
        if cursor.node().kind() == SyntaxKind::opt_partition_clause {
            let mut clause = self.visit_opt_partition_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comments_in_clause(cursor, &mut clause)?;
            clauses.push(clause);
        }

        // cursor -> sort_clause?
        if cursor.node().kind() == SyntaxKind::sort_clause {
            let mut clause = self.visit_sort_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comments_in_clause(cursor, &mut clause)?;
            clauses.push(clause);
        }

        // cursor -> opt_frame_clause?
        if cursor.node().kind() == SyntaxKind::opt_frame_clause {
            let mut clause = self.visit_opt_frame_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comments_in_clause(cursor, &mut clause)?;
            clauses.push(clause);
        }

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::window_specification, src);

        Ok(clauses)
    }

    fn visit_opt_partition_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // opt_partition_clause:
        // - PARTITION BY expr_list

        cursor.goto_first_child();
        // cursor -> PARTITION
        let mut clause = create_clause!(cursor, SyntaxKind::PARTITION);

        cursor.goto_next_sibling();
        // cursor -> BY
        ensure_kind!(cursor, SyntaxKind::BY, src);
        clause.extend_kw(cursor.node());

        cursor.goto_next_sibling();
        // cursor -> comment?
        self.consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> expr_list
        let exprs = self.visit_expr_list(cursor, src)?;

        let sep_lines = SeparatedLines::try_from_expr_list(&exprs)?;
        clause.set_body(Body::SepLines(sep_lines));

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::opt_partition_clause, src);

        Ok(clause)
    }
}
