use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{
        create_clause, ensure_kind,
        expr::{ComplementConfig, ComplementKind},
        Visitor,
    },
};

impl Visitor {
    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    pub(crate) fn visit_select_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // SELECT句の定義
        //    select_clause =
        //        "SELECT"
        //        [select_clause_body]

        // select_clauseは必ずSELECTを子供に持っているはずである
        cursor.goto_first_child();

        // cursor -> SELECT
        let mut clause = create_clause(cursor, src, "SELECT")?;
        cursor.goto_next_sibling();

        // SQL_IDとコメントを消費
        self.consume_or_complement_sql_id(cursor, src, &mut clause);
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> select_caluse_body
        if cursor.node().kind() == "select_clause_body" {
            let body = self.visit_select_clause_body(cursor, src)?;
            clause.set_body(body);
        }

        // cursorをselect_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause")?;

        Ok(clause)
    }

    /// SELECT句の本体をSeparatedLinesで返す
    /// 呼び出し後、cursorはselect_clause_bodyを指している
    fn visit_select_clause_body(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // select_clause_bodyは必ず_aliasable_expressionを子供に持つ
        cursor.goto_first_child();

        // cursor -> _aliasable_expression
        // commaSep1(_aliasable_expression)
        // カラム名ルール(ASがなければASを補完)でエイリアス補完、AS補完を行う
        let complement_config = ComplementConfig::new(ComplementKind::ColumnName, true, true);
        let body = self.visit_comma_sep_alias(cursor, src, Some(&complement_config))?;

        // cursorをselect_clause_bodyに
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause_body")?;

        Ok(body)
    }
}
