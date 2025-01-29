use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind,
        expr::{ComplementConfig, ComplementKind},
        Visitor,
    },
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

        // ASがあれば除去する
        // エイリアス補完は現状行わない
        let complement_config = ComplementConfig::new(ComplementKind::TableName, true, false);
        let body = self.visit_comma_sep_alias(cursor, src, Some(&complement_config))?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause", src)?;

        Ok(clause)
    }
}
