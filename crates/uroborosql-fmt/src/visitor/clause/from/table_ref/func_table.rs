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
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_func_table(): unexpected node appeared. node: {}\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_table, src);

        Ok(func_table)
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

    /// func_alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は func_alias_clause を指している
    pub(crate) fn visit_func_alias_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // func_alias_clause:
        // - alias_clause
        // - AS '(' TableFuncElementList ')'
        // - ColId '(' TableFuncElementList ')'
        // - AS ColId '(' TableFuncElementList ')'

        cursor.goto_first_child();

        // alias_clause なら先に処理して early return
        if cursor.node().kind() == SyntaxKind::alias_clause {
            let alias_clause = self.visit_alias_clause(cursor, src)?;

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);

            return Ok(alias_clause);
        }

        // cursor -> AS?
        let _as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = convert_keyword_case(cursor.node().text());
            cursor.goto_next_sibling();

            Some(as_keyword)
        } else {
            None
        };

        // cursor -> ColId?
        let _col_id = if cursor.node().kind() == SyntaxKind::ColId {
            let col_id = cursor.node().text();
            cursor.goto_next_sibling();

            Some(col_id)
        } else {
            None
        };

        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let table_func_element_list = self.handle_parenthesized_table_func_element_list(cursor, src)?;
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        
        if let Some(col_id) = _col_id {
            // table_func_alias 的な式に追加する
            todo!()
            
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::func_alias_clause, src);

        Ok((_as_keyword, table_func_element_list))
    }
    
    //////////////////////////////////////////////////////////////
    /// 以下仮実装： 返り値は未定
    //////////////////////////////////////////////////////////////

    
    /// 括弧で囲まれた TableFuncElementList を走査する
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    fn handle_parenthesized_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // '(' TableFuncElementList ')'
        // ^^^                      ^^^
        // 呼出し時                  呼出し後
        
        todo!()
    }
    
    fn visit_table_func_element_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // TableFuncElementList:
        // - TableFuncElement ( ',' TableFuncElementList )*
        //
        // this node is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/29
        
        todo!()
    }
    
    fn visit_table_func_element(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // TableFuncElement:
        // - ColId Typename opt_collate_clause?
        todo!()
    }
}
