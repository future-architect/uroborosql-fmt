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

        let mut cond_expr = CondExpr::new(Location::new(cursor.node().range()));

        cursor.goto_first_child();
        // cursor -> "CASE"

        // 大文字小文字情報を保持するために、出現した"CASE"文字列を保持
        let case_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();
        cond_expr.set_case_keyword(case_keyword);

        while cursor.goto_next_sibling() {
            // cursor -> "WHEN" || "ELSE" || "END"
            let kw_node = cursor.node();

            match kw_node.kind() {
                "WHEN" => {
                    let mut when_clause = create_clause(cursor, src, "WHEN")?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> _expression
                    let when_expr = self.format_expr(cursor, src)?;
                    when_clause.set_body(Body::with_expr(when_expr));

                    cursor.goto_next_sibling();
                    // cursor -> comment | "THEN"
                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> "THEN"
                    let mut then_clause = create_clause(cursor, src, "THEN")?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut then_clause)?;

                    // cursor -> _expression
                    let then_expr = self.format_expr(cursor, src)?;
                    then_clause.set_body(Body::with_expr(then_expr));

                    cond_expr.add_when_then_clause(when_clause, then_clause);
                }
                "ELSE" => {
                    let mut else_clause = create_clause(cursor, src, "ELSE")?;
                    cursor.goto_next_sibling();
                    self.consume_comment_in_clause(cursor, src, &mut else_clause)?;

                    // cursor -> _expression
                    let else_expr = self.format_expr(cursor, src)?;
                    else_clause.set_body(Body::with_expr(else_expr));

                    cond_expr.set_else_clause(else_clause);
                }
                "END" => {
                    // 大文字小文字情報を保持するために、出現した"END"文字列を保持
                    let end_keyword = {
                        let tmp_end_keyword = kw_node.utf8_text(src.as_bytes()).unwrap();
                        if tmp_end_keyword.is_empty() {
                            "END"
                        } else {
                            tmp_end_keyword
                        }
                    };

                    cond_expr.set_end_keyword(end_keyword);
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

        cursor.goto_parent();
        ensure_kind(cursor, "conditional_expression")?;

        Ok(cond_expr)
    }
}
