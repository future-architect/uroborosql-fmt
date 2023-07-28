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

    pub(crate) fn visit_type_cast(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        let cast_loc = Location::new(cursor.node().range());

        cursor.goto_first_child();
        // TODO: postgreSQLの::記法への対応

        // CAST関数
        ensure_kind(cursor, "CAST")?;
        let cast_keyword = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

        cursor.goto_next_sibling();
        ensure_kind(cursor, "(")?;
        cursor.goto_next_sibling();

        // キャストされる式
        // 注: キャスト関数の式は alias ノードになっていないので、
        // visit_aliasable_expr では対処できない。
        let expr = self.visit_expr(cursor, src)?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "AS")?;
        let as_keyword = convert_keyword_case(cursor.node().utf8_text(src.as_bytes()).unwrap());

        cursor.goto_next_sibling();

        ensure_kind(cursor, "type")?;
        // 型は特殊な書き方をされていないことを想定し、ソースの文字列をそのまま PrimaryExpr に変換する。
        // 例えば、"CHAR   ( 3    )" などのように、途中に空白を含むような特殊な書き方をした場合、フォーマット結果にもその空白が現れてしまう。
        let type_name = Expr::Primary(Box::new(PrimaryExpr::with_node(
            cursor.node(),
            src,
            PrimaryExprKind::Keyword,
        )));
        cursor.goto_next_sibling();

        ensure_kind(cursor, ")")?;

        // expr AS type を AlignedExpr にする。
        // エイリアスのASとは意味が異なるので、is_alias には false を与える。
        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs(Some(as_keyword), type_name);
        let loc = aligned.loc();

        let args = ColumnList::new(vec![aligned], loc);

        let function = FunctionCall::new(cast_keyword, args, FunctionCallKind::BuiltIn, cast_loc);

        cursor.goto_parent();
        ensure_kind(cursor, "type_cast")?;

        Ok(function)
    }
}
