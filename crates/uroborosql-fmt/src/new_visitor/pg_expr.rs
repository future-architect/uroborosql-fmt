use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Expr, PrimaryExpr},
    error::UroboroSQLFmtError,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};

/*
    postgres の文法における Expression
    - a_expr
        - c_expr
        - ...
    - b_expr
        - ...
    - c_expr
        - columnref
        - AexprConst
        - PARAM opt_indirection
        - '(' a_expr ')' opt_indirection
        - case_expr
        - func_expr
        - select_with_parens
        - select_with_parens indirection
        - EXISTS select_with_parens
        - ARRAY select_with_parens
        - ARRAY array_expr
        - explicit_row
        - implicit_row
        - GROUPING '(' expr_list ')'
*/
impl Visitor {
    // 呼び出した後、cursorは a_expr を指している
    pub(crate) fn visit_a_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::c_expr => self.visit_c_expr(cursor, src)?,
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_a_expr(): unimplemented expression\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;

        Ok(expr)
    }

    fn visit_b_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        unimplemented!()
    }

    fn visit_c_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::columnref => self.visit_columnref(cursor, src)?,
            SyntaxKind::AexprConst => self.visit_aexpr_const(cursor, src)?,
            SyntaxKind::PARAM => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): PARAM is not implemented".to_string(),
                ))
            }
            SyntaxKind::select_with_parens => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): select_with_parens is not implemented".to_string(),
                ))
            }
            SyntaxKind::EXISTS => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): EXISTS is not implemented".to_string(),
                ))
            }
            SyntaxKind::ARRAY => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): ARRAY is not implemented".to_string(),
                ))
            }
            SyntaxKind::explicit_row => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): explicit_row is not implemented".to_string(),
                ))
            }
            SyntaxKind::implicit_row => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): implicit_row is not implemented".to_string(),
                ))
            }
            SyntaxKind::GROUPING => {
                return Err(UroboroSQLFmtError::Unimplemented(
                    "visit_c_expr(): GROUPING is not implemented".to_string(),
                ))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_c_expr(): unexpected syntaxkind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::c_expr, src)?;

        Ok(expr)
    }

    // AexprConst
    fn visit_aexpr_const(
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
            SyntaxKind::Iconst => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::FCONST => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::Sconst => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::BCONST => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::XCONST => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
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
            SyntaxKind::TRUE_P => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::FALSE_P => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            SyntaxKind::NULL_P => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
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

    fn visit_columnref(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // columnref
        // - ColId
        // - ColId indirection
        //   - e.g.: `a.field`, `a.field[1]`

        // cursor -> ColId (必ず存在する)
        cursor.goto_first_child();

        pg_ensure_kind(cursor, SyntaxKind::ColId, src)?;
        let col_id = Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?));

        if cursor.goto_next_sibling() {
            // cursor -> indirection
            // TODO: flatten indirection
            pg_ensure_kind(cursor, SyntaxKind::indirection, src)?;
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_columnref(): indirection is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::columnref, src)?;

        Ok(col_id)
    }
}
