mod a_expr;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{AsteriskExpr, Expr, PrimaryExpr, PrimaryExprKind},
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
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
        let mut columnref_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            // cursor -> indirection
            pg_ensure_kind(cursor, SyntaxKind::indirection, src)?;

            // indirection
            // - indirection_el
            //    - `.` attr_name
            //    - `.` `*`
            //    - `[` a_expr `]`
            //    - `[` opt_slice_bound `:` opt_slice_bound `]`
            //
            // indirection はフラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/7

            let indirection_text = cursor.node().text();

            // 配列アクセスは unimplemented
            if indirection_text.contains('[') {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_columnref(): array access is not implemented\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            // indirection にあたるテキストから空白文字を除去し、そのまま追加している
            let whitespace_removed = indirection_text
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            columnref_text.push_str(&whitespace_removed);
        }

        // アスタリスクが含まれる場合はAsteriskExprに変換する
        let expr = if columnref_text.contains('*') {
            AsteriskExpr::new(
                convert_identifier_case(&columnref_text),
                cursor.node().range().into(),
            )
            .into()
        } else {
            PrimaryExpr::new(
                convert_identifier_case(&columnref_text),
                cursor.node().range().into(),
            )
            .into()
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::columnref, src)?;

        Ok(expr)
    }
}
