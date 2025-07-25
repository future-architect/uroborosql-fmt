use crate::{
    cst::{Expr, FunctionCall},
    error::UroboroSQLFmtError,
    visitor::{error_annotation_from_cursor, Visitor},
};
use postgresql_cst_parser::tree_sitter::TreeCursor;

impl Visitor {
    /// 呼出し後、cursor は func_table を指している
    pub(crate) fn visit_func_table(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        // func_table
        // - func_expr_windowless opt_ordinality
        // - ROWS FROM '(' rowsfrom_list ')' opt_ordinality

        Err(UroboroSQLFmtError::Unimplemented(format!(
            "visit_func_table(): func_table node appeared. Table function calls are not implemented yet.\n{}",
            error_annotation_from_cursor(cursor, src)
        )))
    }

    /// func_alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は func_alias_clause を指している
    pub(crate) fn visit_func_alias_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // func_alias_clause:
        // - alias_clause
        // - AS? ColId? '(' TableFuncElementList ')'

        Err(UroboroSQLFmtError::Unimplemented(format!(
            "visit_func_alias_clause(): func_alias_clause node appeared. Table function alias clauses are not implemented yet.\n{}",
            error_annotation_from_cursor(cursor, src)
        )))
    }
}
