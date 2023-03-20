mod clause;
mod expr;
mod statement;

use tree_sitter::{Node, TreeCursor};

pub(crate) const COMMENT: &str = "comment";

use crate::cst::*;

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub(crate) depth: usize,
}

pub(crate) struct Formatter {
    state: FormatterState,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub(crate) fn new() -> Formatter {
        Formatter {
            state: FormatterState { depth: 0 },
        }
    }

    /// sqlソースファイルをフォーマット用構造体に変形する
    pub(crate) fn format_sql(
        &mut self,
        node: Node,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        // CSTを走査するTreeCursorを生成する
        // ほかの関数にはこのcursorの可変参照を渡す
        let mut cursor = node.walk();

        self.format_source(&mut cursor, src)
    }

    // ネストを1つ深くする
    fn nest(&mut self) {
        self.state.depth += 1;
    }

    // ネストを1つ浅くする
    fn unnest(&mut self) {
        self.state.depth -= 1;
    }

    /// source_file
    /// 呼び出し終了後、cursorはsource_fileを指している
    fn format_source(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        // source_file -> _statement*
        let mut source: Vec<Statement> = vec![];

        if !cursor.goto_first_child() {
            // source_fileに子供がない、つまり、ソースファイルが空である場合
            // todo
            return Err(UroboroSQLFmtError::UnimplementedError(format!(
                "format_source(): source_file has no child \nnode_kind: {}\n{:#?}",
                cursor.node().kind(),
                cursor.node().range(),
            )));
        }

        // ソースファイル先頭のコメントを保存するバッファ
        let mut comment_buf: Vec<Comment> = vec![];

        // 複数のStatement間のコメントの位置を決定するために使用する
        // 文を読んだが、対応するセミコロンを読んでいない場合はtrue、そうでない場合false
        let mut above_semi = true;

        loop {
            let kind = cursor.node().kind();

            if kind.ends_with("_statement") {
                let mut stmt = match kind {
                    "select_statement" => self.format_select_stmt(cursor, src)?,
                    "delete_statement" => self.format_delete_stmt(cursor, src)?,
                    "update_statement" => self.format_update_stmt(cursor, src)?,
                    "insert_statement" => self.format_insert_stmt(cursor, src)?,
                    // todo
                    _ => {
                        return Err(UroboroSQLFmtError::UnimplementedError(format!(
                            "format_source(): Unimplemented statement\nnode_kind: {}\n{:#?}",
                            cursor.node().kind(),
                            cursor.node().range(),
                        )))
                    }
                };

                // コメントが以前にあれば先頭に追加
                comment_buf
                    .iter()
                    .cloned()
                    .for_each(|c| stmt.add_comment(c));
                comment_buf.clear();

                source.push(stmt);
                above_semi = true;
            } else if kind == COMMENT {
                let comment = Comment::new(cursor.node(), src);

                if !source.is_empty() && above_semi {
                    let last_stmt = source.last_mut().unwrap();
                    // すでにstatementがある場合、末尾に追加
                    last_stmt.add_comment_to_child(comment)?;
                } else {
                    // まだstatementがない場合、バッファに詰めておく
                    comment_buf.push(comment);
                }
            } else if kind == ";" {
                above_semi = false;
                if let Some(last) = source.last_mut() {
                    last.set_semi(true);
                }
                // tree-sitter-sqlでは、;の上に文がない場合syntax errorになる
            }

            if !cursor.goto_next_sibling() {
                // 次の子供がいない場合、終了
                break;
            }
        }
        // cursorをsource_fileに戻す
        cursor.goto_parent();

        Ok(source)
    }

    /// _aliasable_expressionが,で区切られた構造をBodyにして返す
    fn format_comma_sep_alias(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        omit_as: bool,
    ) -> Result<Body, UroboroSQLFmtError> {
        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", omit_as);

        // commaSep(_aliasable_expression)
        let alias = self.format_aliasable_expr(cursor, src)?;
        separated_lines.add_expr(alias);

        // ("," _aliasable_expression)*
        while cursor.goto_next_sibling() {
            // cursor -> , または comment または _aliasable_expression
            match cursor.node().kind() {
                "," => {
                    cursor.goto_next_sibling();
                    // _aliasable_expression
                    let alias = self.format_aliasable_expr(cursor, src)?;
                    separated_lines.add_expr(alias);
                }
                COMMENT => {
                    separated_lines.add_comment_to_child(Comment::new(cursor.node(), src))?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                                    "format_comma_sep_alias(): expected node is ',' or COMMENT, but actual {}\n{:?}",
                                    cursor.node().kind(),
                                    cursor.node().range()
                                )))
                },
            }
        }

        Ok(Body::SepLines(separated_lines))
    }

    /// カーソルが指すノードがSQL_IDであれば、clauseに追加する
    fn consume_sql_id(&mut self, cursor: &mut TreeCursor, src: &str, clause: &mut Clause) {
        if cursor.node().kind() != COMMENT {
            return;
        }

        let comment = Comment::new(cursor.node(), src);

        // _SQL_ID_であれば追加
        if comment.is_sql_id_comment() {
            clause.set_sql_id(comment);
            cursor.goto_next_sibling();
        }
    }

    /// カーソルが指すノードがコメントであれば、コメントを消費してclauseに追加する
    fn consume_comment_in_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clause: &mut Clause,
    ) -> Result<(), UroboroSQLFmtError> {
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            clause.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        Ok(())
    }
}

/// cursorが指定した種類のノードを指しているかどうかをチェックする関数
/// 期待しているノードではない場合、エラーを返す
fn ensure_kind<'a>(
    cursor: &'a TreeCursor<'a>,
    kind: &'a str,
) -> Result<&'a TreeCursor<'a>, UroboroSQLFmtError> {
    if cursor.node().kind() != kind {
        Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
            "ensure_kind(): excepted node is {}, but actual {}\n{:#?}",
            kind,
            cursor.node().kind(),
            cursor.node().range()
        )))
    } else {
        Ok(cursor)
    }
}

/// keyword の Clauseを生成する関数。
/// 呼び出し後の cursor はキーワードの最後のノードを指す。
/// cursor のノードがキーワードと異なっていたら UroboroSQLFmtErrorを返す。
/// 複数の語からなるキーワードは '_' で区切られており、それぞれのノードは同じ kind を持っている。
///
/// 例: "ORDER_BY" は
///     (content: "ORDER", kind: "ORDER_BY")
///     (content: "BY", kind: "ORDER_BY")
/// というノードになっている。
fn create_clause(
    cursor: &mut TreeCursor,
    src: &str,
    keyword: &str,
    depth: usize,
) -> Result<Clause, UroboroSQLFmtError> {
    ensure_kind(cursor, keyword)?;
    let mut clause = Clause::new(cursor.node(), src, depth);

    for _ in 1..keyword.split('_').count() {
        cursor.goto_next_sibling();
        ensure_kind(cursor, keyword)?;
        clause.extend_kw(cursor.node(), src);
    }

    Ok(clause)
}
