use tree_sitter::TreeCursor;

use crate::formatter::{create_clause, ensure_kind, Formatter, COMMENT};

use crate::cst::*;

impl Formatter {
    /// CASE式をフォーマットする
    /// 呼び出し後、cursorはconditional_expressionを指す
    pub(crate) fn format_cond_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<CondExpr, UroboroSQLFmtError> {
        // conditional_expression ->
        //     "CASE"
        //     ("WHEN" expression "THEN" expression)*
        //     ("ELSE" expression)?
        //     "END"

        let mut cond_expr = CondExpr::new(Location::new(cursor.node().range()), self.state.depth);

        // CASE, WHEN(, THEN, ELSE)キーワードの分で2つネストが深くなる
        // TODO: ネストの深さの計算をrender()メソッドで行う変更
        self.nest();
        self.nest();

        cursor.goto_first_child();
        // cursor -> "CASE"

        while cursor.goto_next_sibling() {
            // cursor -> "WHEN" || "ELSE" || "END"
            let kw_node = cursor.node();

            match kw_node.kind() {
                "WHEN" => {
                    let mut when_clause = create_clause(cursor, src, "WHEN", self.state.depth)?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> _expression
                    let when_expr = self.format_expr(cursor, src)?;
                    when_clause.set_body(Body::with_expr(when_expr, self.state.depth));

                    cursor.goto_next_sibling();
                    // cursor -> comment | "THEN"
                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> "THEN"
                    let mut then_clause = create_clause(cursor, src, "THEN", self.state.depth)?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut then_clause)?;

                    // cursor -> _expression
                    let then_expr = self.format_expr(cursor, src)?;
                    then_clause.set_body(Body::with_expr(then_expr, self.state.depth));

                    cond_expr.add_when_then_clause(when_clause, then_clause);
                }
                "ELSE" => {
                    let mut else_clause = create_clause(cursor, src, "ELSE", self.state.depth)?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut else_clause)?;

                    // cursor -> _expression
                    let else_expr = self.format_expr(cursor, src)?;
                    else_clause.set_body(Body::with_expr(else_expr, self.state.depth));

                    cond_expr.set_else_clause(else_clause);
                }
                "END" => {
                    break;
                }
                COMMENT => {
                    // カーソルを覚えておく
                    let current_cursor = cursor.clone();

                    // バインドパラメータである可能性があるため、調べる
                    match self.format_expr(cursor, src) {
                        Ok(expr) => {
                            // expression としてフォーマットできた場合は、単純CASE式としてセットする
                            // ここで、単純CASE式の条件以外の部分では、バインドパラメータを持つ式は現れないことを想定する。
                            cond_expr.set_expr(expr);
                        }
                        Err(_) => {
                            // バインドパラメータではない場合、カーソルを戻してからコメントをセットする。
                            *cursor = current_cursor;
                            let comment_node = cursor.node();
                            let comment = Comment::new(comment_node, src);

                            // 行末コメントを式にセットする
                            cond_expr.set_trailing_comment(comment)?;
                        }
                    }
                }
                _ => {
                    // 単純CASE式とみなす
                    let expr = self.format_expr(cursor, src)?;
                    cond_expr.set_expr(expr);
                }
            }
        }

        self.unnest();
        self.unnest();

        cursor.goto_parent();
        ensure_kind(cursor, "conditional_expression")?;

        Ok(cond_expr)
    }
}
