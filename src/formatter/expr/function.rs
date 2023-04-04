//! 関数呼び出しに関するフォーマットを定義

use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    formatter::{ensure_kind, Formatter},
};

impl Formatter {
    pub(crate) fn format_function_call(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        let function_call_loc = Location::new(cursor.node().range());
        cursor.goto_first_child();

        // "LATERAL"は未対応

        // 関数名
        let function_name = cursor.node().utf8_text(src.as_bytes()).unwrap();
        cursor.goto_next_sibling();

        ensure_kind(cursor, "(")?;
        let args = self.format_function_call_arguments(cursor, src)?;
        cursor.goto_next_sibling();

        let mut func_call = FunctionCall::new(function_name, &args, function_call_loc);

        // TODO: filter

        if cursor.node().kind() == "over_clause" {
            func_call.set_over_window_definition(&self.format_over_clause(cursor, src)?);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        ensure_kind(cursor, "function_call")?;

        Ok(func_call)
    }

    /// 関数呼び出しの引数をフォーマット
    /// 引数の前に現れるALL/DISTINCTと、引数の後に現れるorder byには未対応
    pub(crate) fn format_function_call_arguments(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Expr>, UroboroSQLFmtError> {
        let mut args: Vec<Expr> = vec![];
        loop {
            if !cursor.goto_next_sibling() {
                return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                    "format_function_call_arguments(): expected '('\nnode kind{}\n{:?}",
                    cursor.node().kind(),
                    cursor.node().range()
                )));
            }

            match cursor.node().kind() {
                ")" => {
                    break;
                }
                "," => {
                    continue;
                }
                // TODO: 引数のORDER BY句、ALL、DISTINCTに対応する
                "order_by_clause" | "ALL" | "DISTINCT" => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_function_call_arguments():  unimplemented node\nnode kind{}\n{:?}",
                        cursor.node().kind(),
                        cursor.node().range()
                    )))
                }
                _ => {
                    // TODO: 関数呼び出しの引数の部分に、コメントを許容できるようにする
                    let expr = self.format_expr(cursor, src)?;
                    args.push(expr);
                }
            }
        }

        Ok(args)
    }

    fn format_over_clause(
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
                self.format_simple_clause(cursor, src, "partition_by_clause", "PARTITION_BY")?;
            cursor.goto_next_sibling();
            self.consume_comment_in_clause(cursor, src, &mut clause)?;
            clauses.push(clause);
        };

        if cursor.node().kind() == "order_by_clause" {
            let mut clause = self.format_order_by_clause(cursor, src)?;
            cursor.goto_next_sibling();
            self.consume_comment_in_clause(cursor, src, &mut clause)?;
            clauses.push(clause);
        }

        if cursor.node().kind() == "frame_clause" {
            let mut clause = self.format_frame_clause(cursor, src)?;
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

    pub(crate) fn format_type_cast(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<FunctionCall, UroboroSQLFmtError> {
        let cast_loc = Location::new(cursor.node().range());

        cursor.goto_first_child();
        // TODO: postgreSQLの::記法への対応

        // CAST関数
        ensure_kind(cursor, "CAST")?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "(")?;
        cursor.goto_next_sibling();

        // キャストされる式
        // 注: キャスト関数の式は alias ノードになっていないので、
        // format_aliasable_expr では対処できない。
        let expr = self.format_expr(cursor, src)?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "AS")?;
        cursor.goto_next_sibling();

        ensure_kind(cursor, "type")?;
        // 型は特殊な書き方をされていないことを想定し、ソースの文字列をそのまま PrimaryExpr に変換する。
        // 例えば、"CHAR   ( 3    )" などのように、途中に空白を含むような特殊な書き方をした場合、フォーマット結果にもその空白が現れてしまう。
        let type_name = Expr::Primary(Box::new(PrimaryExpr::with_node(cursor.node(), src)));
        cursor.goto_next_sibling();

        ensure_kind(cursor, ")")?;

        // expr AS type を AlignedExpr にする。
        // エイリアスのASとは意味が異なるので、is_alias には false を与える。
        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs("AS", type_name);
        let expr_as_type = Expr::Aligned(Box::new(aligned));

        let function = FunctionCall::new("CAST", &[expr_as_type], cast_loc);

        cursor.goto_parent();
        ensure_kind(cursor, "type_cast")?;

        Ok(function)
    }
}
