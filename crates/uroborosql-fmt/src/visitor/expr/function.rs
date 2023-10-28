//! 関数呼び出しに関するフォーマットを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT},
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

        ensure_kind(cursor, "(", src)?;

        let args = self.visit_function_call_args(cursor, src)?;
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
        ensure_kind(cursor, "function_call", src)?;

        Ok(func_call)
    }

    fn visit_over_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Clause>, UroboroSQLFmtError> {
        cursor.goto_first_child();
        // over
        ensure_kind(cursor, "OVER", src)?;
        cursor.goto_next_sibling();

        // window_definition
        ensure_kind(cursor, "window_definition", src)?;
        cursor.goto_first_child();

        ensure_kind(cursor, "(", src)?;

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

        ensure_kind(cursor, ")", src)?;

        cursor.goto_parent();
        // cursor -> window_definition

        cursor.goto_parent();
        ensure_kind(cursor, "over_clause", src)?;

        Ok(clauses)
    }

    /// 関数の引数をFunctionCallArgsで返す
    /// 引数は "(" [ ALL | DISTINCT ] expression [ , ... ] [ order_by_clause ] ")" という構造になっている
    pub(crate) fn visit_function_call_args(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCallArgs, UroboroSQLFmtError> {
        let mut function_call_args =
            FunctionCallArgs::new(vec![], Location::new(cursor.node().range()));

        ensure_kind(cursor, "(", src)?;

        cursor.goto_next_sibling();

        // 引数が空の場合
        if cursor.node().kind() == ")" {
            return Ok(function_call_args);
        }

        match cursor.node().kind() {
            "ALL" | "DISTINCT" => {
                let all_distinct_clause = create_clause(cursor, src, cursor.node().kind())?;

                function_call_args.set_all_distinct(all_distinct_clause);

                cursor.goto_next_sibling();
            }
            _ => {}
        }

        let first_expr = self.visit_expr(cursor, src)?.to_aligned();
        function_call_args.add_expr(first_expr);

        // [ , ... ] [ order_by_clause ] ")"
        while cursor.goto_next_sibling() {
            function_call_args.append_loc(Location::new(cursor.node().range()));

            match cursor.node().kind() {
                COMMA => {
                    cursor.goto_next_sibling();
                    let expr = self.visit_expr(cursor, src)?.to_aligned();
                    function_call_args.add_expr(expr);
                }
                ")" => break,
                COMMENT => {
                    // 末尾コメントを想定する
                    let comment = Comment::new(cursor.node(), src);
                    function_call_args.set_trailing_comment(comment)?
                }
                "order_by_clause" => {
                    let order_by = self.visit_order_by_clause(cursor, src)?;
                    function_call_args.set_order_by(order_by);
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_function_call_args(): Unexpected node\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        Ok(function_call_args)
    }
}
