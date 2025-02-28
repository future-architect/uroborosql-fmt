mod a_expr;
mod c_expr;

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
}
