use crate::{
    cst::{Expr, FunctionTable},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

impl Visitor {
    /// 呼出し後、cursor は func_table を指している
    pub(crate) fn visit_func_table(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionTable, UroboroSQLFmtError> {
        // func_table
        // - func_expr_windowless opt_ordinality
        // - ROWS FROM '(' rowsfrom_list ')' opt_ordinality

        let loc = cursor.node().range().into();

        cursor.goto_first_child();

        let func_table = match cursor.node().kind() {
            SyntaxKind::func_expr_windowless => {
                let func_expr = self.visit_func_expr_windowless(cursor, src)?;

                cursor.goto_next_sibling();

                // cursor -> opt_ordinality?
                let with_ordinality = if cursor.node().kind() == SyntaxKind::opt_ordinality {
                    Some(self.visit_opt_ordinality(cursor)?)
                } else {
                    None
                };

                FunctionTable::new(func_expr, with_ordinality, loc)
            }
            SyntaxKind::ROWS => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_table(): ROWS node appeared. 'ROWS FROM (rowsfrom_list)' pattern is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_func_table(): unimplemented func_table node appeared. func_table node is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_table, src);

        Ok(func_table)
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

    fn visit_opt_ordinality(
        &mut self,
        cursor: &mut TreeCursor,
    ) -> Result<String, UroboroSQLFmtError> {
        // opt_ordinality:
        // - WITH_LA ORDINALITY

        let text = cursor.node().text();
        Ok(convert_keyword_case(text))
    }
}
