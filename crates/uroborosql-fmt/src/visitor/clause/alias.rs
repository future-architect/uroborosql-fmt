mod name_list;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    config::CONFIG,
    cst::{table_function_alias::TableFuncAlias, ColumnList, Expr, Location, PrimaryExpr},
    error::UroboroSQLFmtError,
    util::convert_identifier_case,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    /// alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は alias_clause を指している
    pub(crate) fn visit_alias_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // alias_clause
        // - [AS] ColId ['(' name_list ')']

        // cursor -> alias_clause
        ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        cursor.goto_first_child();
        // cursor -> AS?
        let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = cursor.node().text().to_string();
            cursor.goto_next_sibling();

            // remove_table_as_keyword が有効ならば AS を除去
            if CONFIG.read().unwrap().remove_table_as_keyword {
                None
            } else {
                Some(as_keyword)
            }
        } else {
            None
        };

        // cursor -> ColId
        let col_id = convert_identifier_case(cursor.node().text());
        let col_id_loc = Location::from(cursor.node().range());
        cursor.goto_next_sibling();

        // cursor -> '('?
        let expr = if cursor.node().kind() == SyntaxKind::LParen {
            let list = self.handle_parenthesized_name_list(cursor, src)?;
            let column_list = ColumnList::from(list);

            let table_func_alias = TableFuncAlias::new(Some(col_id), column_list, col_id_loc);

            Expr::TableFuncAlias(Box::new(table_func_alias))
        } else {
            let primary = PrimaryExpr::new(col_id, col_id_loc);
            Expr::Primary(Box::new(primary))
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        Ok((as_keyword, expr))
    }
}
