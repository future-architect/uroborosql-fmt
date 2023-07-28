use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, ComplementKind, Visitor},
};

impl Visitor {
    /// FROM句をClause構造体で返す
    pub(crate) fn visit_from_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clauseは必ずFROMを子供に持つ
        cursor.goto_first_child();

        // cursor -> FROM
        let mut clause = create_clause(cursor, src, "FROM")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> aliasable_expression
        // commaSep1(_aliasable_expression)
        // テーブル名ルール(ASがあれば省略)で補完を行う。
        // エイリアス補完は現状行わない
        let body =
            self.visit_comma_sep_alias(cursor, src, Some(&ComplementKind::TableName), true, false)?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause")?;

        Ok(clause)
    }
}
