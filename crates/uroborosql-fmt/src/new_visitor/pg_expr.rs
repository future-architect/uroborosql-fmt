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
            _ => unimplemented!(),
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
            SyntaxKind::columnref => unimplemented!("columnref"),
            SyntaxKind::AexprConst => self.visit_aexpr_const(cursor, src)?,
            SyntaxKind::PARAM => unimplemented!("PARAM"),
            SyntaxKind::select_with_parens => unimplemented!("select_with_parens"),
            SyntaxKind::EXISTS => unimplemented!("EXISTS"),
            SyntaxKind::ARRAY => unimplemented!("ARRAY"),
            SyntaxKind::explicit_row => unimplemented!("explicit_row"),
            SyntaxKind::implicit_row => unimplemented!("implicit_row"),
            SyntaxKind::GROUPING => unimplemented!("GROUPING"),
            // return unexpected syntaxkind
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
        // とりあえず Iconst だけ実装する
        let expr = match cursor.node().kind() {
            SyntaxKind::Iconst => {
                Expr::Primary(Box::new(PrimaryExpr::with_pg_node(cursor.node())?))
            }
            _ => unimplemented!("AexprConst"),
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::AexprConst, src)?;

        Ok(expr)
    }
}
