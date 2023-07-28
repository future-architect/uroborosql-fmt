use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, Visitor},
};

impl Visitor {
    /// キーワードとカンマで区切られた式からなる、単純な句をフォーマットする。
    /// 引数の `clause_node_name` に句のノード名を、`clause_keyword` にキーワードを与える。
    /// 例えば、`visit_simple_clause(cursor, src, "having_clause", "HAVING")` のように使用する。
    ///
    /// ```sql
    /// KEYWORD
    ///     EXPR1
    /// ,   EXPR2
    /// ...
    /// ```
    pub(crate) fn visit_simple_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clause_node_name: &str,
        clause_keyword: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let mut clause = create_clause(cursor, src, clause_keyword)?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // SimpleClauseは現状補完を行わない
        let body = self.visit_comma_sep_alias(cursor, src, None, false, false)?;

        clause.set_body(body);

        // cursorを戻す
        cursor.goto_parent();
        ensure_kind(cursor, clause_node_name)?;

        Ok(clause)
    }
}
