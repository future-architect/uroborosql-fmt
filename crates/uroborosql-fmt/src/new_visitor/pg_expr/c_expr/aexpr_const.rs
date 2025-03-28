use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Expr, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    new_visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;
impl Visitor {
    pub fn visit_aexpr_const(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // AexprConst
        // - Iconst
        //   - ICONST
        // - FCONST
        // - Sconst
        //   - SCONST
        // - BCONST
        // - XCONST
        // - func_name Sconst
        // - func_name '(' func_arg_list opt_sort_clause ')' Sconst
        // - ConstTypename Sconst
        // - ConstInterval Sconst opt_interval
        // - ConstInterval '(' Iconst ')' Sconst
        // - TRUE_P
        // - FALSE_P
        // - NULL_P

        cursor.goto_first_child();
        let expr = match cursor.node().kind() {
            SyntaxKind::Iconst
            | SyntaxKind::FCONST
            | SyntaxKind::Sconst
            | SyntaxKind::BCONST
            | SyntaxKind::XCONST => {
                PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?.into()
            }
            SyntaxKind::func_name => {
                // func_name Sconst
                // func_name '(' func_arg_list opt_sort_clause ')' Sconst
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_aexpr_const(): func_name is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::ConstTypename => {
                // ConstTypename Sconst
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_aexpr_const(): ConstTypename is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::ConstInterval => {
                // ConstInterval Sconst opt_interval
                // ConstInterval '(' Iconst ')' Sconst
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_aexpr_const(): ConstInterval is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::TRUE_P | SyntaxKind::FALSE_P | SyntaxKind::NULL_P => {
                PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Keyword)?.into()
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_aexpr_const(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::AexprConst, src)?;

        Ok(expr)
    }
}
