mod func_application;
mod func_expr_common_subexpr;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::Expr,
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    pub fn visit_func_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // func_expr
        // - func_application + within_group_clause + filter_clause + over_clause
        // - func_expr_common_subexpr
        // - json_aggregate_func + filter_clause + over_clause

        cursor.goto_first_child();
        // cursor -> func_application | func_expr_common_subexpr | json_aggregate_func

        let func = match cursor.node().kind() {
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
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_func_expr(): filter_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
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
}
