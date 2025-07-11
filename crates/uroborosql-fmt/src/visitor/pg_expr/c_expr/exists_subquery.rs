use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{ExistsSubquery, Expr, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;

impl Visitor {
    /// EXISTS サブクエリをフォーマットする
    /// 呼出し後、cursor は最後の要素（select_with_parens）を指している
    pub fn handle_exists_subquery_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ExistsSubquery, UroboroSQLFmtError> {
        // EXISTS select_with_parens
        // ^      ^
        // |      |
        // |      └ 呼出後
        // └ 呼出時

        let exists_loc = Location::from(
            cursor
                .node()
                .parent()
                .expect("exists_subquery: parent not found")
                .range(),
        );

        // cursor -> EXISTS
        pg_ensure_kind!(cursor, SyntaxKind::EXISTS, src);

        let exists_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> select_with_parens

        let select_expr = self.visit_select_with_parens(cursor, src)?;

        let select_subexpr = match select_expr {
            Expr::Sub(subexpr) => *subexpr,
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_exists_subquery(): select_expr is not a select expression\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        let exists_subquery = ExistsSubquery::new(&exists_keyword, select_subexpr, exists_loc);

        pg_ensure_kind!(cursor, SyntaxKind::select_with_parens, src);

        Ok(exists_subquery)
    }
}
