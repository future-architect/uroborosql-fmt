//! 関数呼び出しに関するフォーマットを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, Visitor},
};

impl Visitor {
    pub(crate) fn visit_function_call(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        let function_call_loc = Location::new(cursor.node().range());
        cursor.goto_first_child();

        // "LATERAL"は未対応

        // 関数名
        let function_name = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());
        cursor.goto_next_sibling();

        ensure_kind(cursor, "(")?;
        let args = self.visit_column_list(cursor, src)?;
        cursor.goto_next_sibling();

        let mut func_call = FunctionCall::new(
            function_name,
            args,
            FunctionCallKind::UserDefined,
            function_call_loc,
        );

        // TODO: filter

        if cursor.node().kind() == "over_clause" {
            // 大文字小文字情報を保持するために、出現した"OVER"文字列を保持
            // over_clauseの1つ目の子供が"OVER"であるはずなので取得
            let over_keyword = convert_keyword_case(
                cursor
                    .node()
                    .child(0)
                    .unwrap()
                    .utf8_text(src.as_bytes())
                    .unwrap(),
            );
            func_call.set_over_keyword(&over_keyword);

            func_call.set_over_window_definition(&self.visit_over_clause(cursor, src)?);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        ensure_kind(cursor, "function_call")?;

        Ok(func_call)
    }

    fn visit_over_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        cursor.goto_first_child();
        // over
        ensure_kind(cursor, "OVER")?;
        cursor.goto_next_sibling();

        // window_definition
        ensure_kind(cursor, "window_definition")?;
        cursor.goto_first_child();

        ensure_kind(cursor, "(")?;

        cursor.goto_next_sibling();

        let mut clauses: Vec<Clause> = vec![];

        if cursor.node().kind() == "partition_by_clause" {
            let mut clause =
                self.visit_simple_clause(cursor, src, "partition_by_clause", "PARTITION_BY")?;
            cursor.goto_next_sibling();
            self.consume_comment_in_clause(cursor, src, &mut clause)?;
            clauses.push(clause);
        };

        if cursor.node().kind() == "order_by_clause" {
            let mut clause = self.visit_order_by_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comment_in_clause(cursor, src, &mut clause)?;
            clauses.push(clause);
        }

        if cursor.node().kind() == "frame_clause" {
            let mut clause = self.visit_frame_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comment_in_clause(cursor, src, &mut clause)?;
            clauses.push(clause);
        }

        ensure_kind(cursor, ")")?;

        cursor.goto_parent();
        // cursor -> window_definition

        cursor.goto_parent();
        ensure_kind(cursor, "over_clause")?;

        Ok(clauses)
    }
}
