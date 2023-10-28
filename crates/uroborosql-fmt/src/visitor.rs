mod clause;
mod expr;
mod statement;

use tree_sitter::{Node, TreeCursor};

pub(crate) const COMMENT: &str = "comment";
pub(crate) const COMMA: &str = ",";

use crate::{config::CONFIG, cst::*, error::UroboroSQLFmtError, util::convert_identifier_case};

use self::expr::ComplementConfig;

pub(crate) struct Visitor {
    /// select文、insert文などが複数回出てきた際に1度だけSQL_IDを補完する、という処理を実現するためのフラグ
    should_complement_sql_id: bool,
}

impl Default for Visitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Visitor {
    /// CONFIGファイルを見て、補完フラグがtrueの場合は`should_complement_sql_id`をtrueにして初期化する
    pub(crate) fn new() -> Visitor {
        Visitor {
            should_complement_sql_id: CONFIG.read().unwrap().complement_sql_id,
        }
    }

    /// sqlソースファイルをフォーマット用構造体に変形する
    pub(crate) fn visit_sql(
        &mut self,
        node: Node,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        // CSTを走査するTreeCursorを生成する
        // ほかの関数にはこのcursorの可変参照を渡す
        let mut cursor = node.walk();

        self.visit_source(&mut cursor, src)
    }

    /// source_file
    /// 呼び出し終了後、cursorはsource_fileを指している
    fn visit_source(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<Statement>, UroboroSQLFmtError> {
        // source_file -> _statement*
        let mut source: Vec<Statement> = vec![];

        if !cursor.goto_first_child() {
            // source_fileに子供がない、つまり、ソースファイルが空である場合
            // todo
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_source(): source_file has no child \nnode_kind: {}\n{:#?}",
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
                    "select_statement" => self.visit_select_stmt(cursor, src)?,
                    "delete_statement" => self.visit_delete_stmt(cursor, src)?,
                    "update_statement" => self.visit_update_stmt(cursor, src)?,
                    "insert_statement" => self.visit_insert_stmt(cursor, src)?,
                    // todo
                    _ => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_source(): Unimplemented statement\n{}",
                            create_error_info(cursor, src)
                        )));
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
    fn visit_comma_sep_alias(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        // エイリアス/AS補完に関する設定
        // Noneの場合は補完を行わない
        complement_config: Option<&ComplementConfig>,
    ) -> Result<Body, UroboroSQLFmtError> {
        let mut separated_lines = SeparatedLines::new();

        // commaSep(_aliasable_expression)
        let alias = self.visit_aliasable_expr(cursor, src, complement_config)?;
        separated_lines.add_expr(alias, None, vec![]);

        // ("," _aliasable_expression)*
        while cursor.goto_next_sibling() {
            // cursor -> , または comment または _aliasable_expression
            match cursor.node().kind() {
                // tree-sitter-sqlにより、構文エラーは検出されるはずなので、"," は読み飛ばしてもよい。
                "," => {}
                COMMENT => {
                    let comment_node = cursor.node();
                    let comment = Comment::new(comment_node, src);

                    // tree-sitter-sqlの性質上、コメントが最後の子供になることはないはずなので、panicしない。
                    let sibling_node = cursor.node().next_sibling().unwrap();

                    // コメントノードがバインドパラメータであるかを判定し、バインドパラメータならば式として処理し、
                    // そうでなければ単にコメントとして処理する。
                    if comment.is_block_comment()
                        && comment
                            .loc()
                            .is_next_to(&Location::new(sibling_node.range()))
                    {
                        let alias = self.visit_aliasable_expr(cursor, src, complement_config)?;
                        separated_lines.add_expr(alias, Some(COMMA.to_string()), vec![]);
                    } else {
                        separated_lines.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    let alias = self.visit_aliasable_expr(cursor, src, complement_config)?;
                    separated_lines.add_expr(alias, Some(COMMA.to_string()), vec![]);
                }
            }
        }

        Ok(Body::SepLines(separated_lines))
    }

    /// identifierが,で区切られた構造をBodyにして返す
    /// 呼び出し後、cursorは区切られた構造の次の要素を指す
    fn visit_comma_sep_identifier(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        let mut separated_lines = SeparatedLines::new();

        // commaSep(identifier)
        let identifier = self.visit_expr(cursor, src)?;
        separated_lines.add_expr(identifier.to_aligned(), None, vec![]);

        // ("," identifier)*
        while cursor.goto_next_sibling() {
            // cursor -> , または comment または identifier
            match cursor.node().kind() {
                // tree-sitter-sqlにより、構文エラーは検出されるはずなので、"," は読み飛ばしてもよい。
                "," => {}
                COMMENT => {
                    let comment_node = cursor.node();
                    let comment = Comment::new(comment_node, src);

                    // tree-sitter-sqlの性質上、コメントが最後の子供になることはないはずなので、panicしない。
                    let sibling_node = cursor.node().next_sibling().unwrap();

                    // コメントノードがバインドパラメータであるかを判定し、バインドパラメータならば式として処理し、
                    // そうでなければ単にコメントとして処理する。
                    if comment.is_block_comment()
                        && comment
                            .loc()
                            .is_next_to(&Location::new(sibling_node.range()))
                    {
                        let identifier = self.visit_expr(cursor, src)?;
                        separated_lines.add_expr(
                            identifier.to_aligned(),
                            Some(COMMA.to_string()),
                            vec![],
                        );
                    } else {
                        separated_lines.add_comment_to_child(comment)?;
                    }
                }
                "identifier" => {
                    let identifier = self.visit_expr(cursor, src)?;
                    separated_lines.add_expr(
                        identifier.to_aligned(),
                        Some(COMMA.to_string()),
                        vec![],
                    );
                }
                _ => {
                    break;
                }
            }
        }

        Ok(Body::SepLines(separated_lines))
    }

    /// カーソルが指すノードがSQL_IDであれば、clauseに追加する
    /// もし (_SQL_ID_が存在していない) && (_SQL_ID_がまだ出現していない) && (_SQL_ID_の補完がオン)
    /// の場合は補完する
    fn consume_or_complement_sql_id(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        clause: &mut Clause,
    ) {
        if cursor.node().kind() == COMMENT {
            let text = cursor.node().utf8_text(src.as_bytes()).unwrap();

            if SqlID::is_sql_id(text) {
                clause.set_sql_id(SqlID::new(text.to_string()));
                cursor.goto_next_sibling();
                self.should_complement_sql_id = false;

                return;
            }
        }

        // SQL_IDがない、かつSQL補完フラグがtrueの場合、補完する
        if self.should_complement_sql_id {
            clause.set_sql_id(SqlID::new("/* _SQL_ID_ */".to_string()));
            self.should_complement_sql_id = false;
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
        Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
            "ensure_kind(): excepted node is {}, but actual {}\n{:#?}",
            kind,
            cursor.node().kind(),
            cursor.node().range()
        )))
    } else {
        Ok(cursor)
    }
}

/// エイリアス補完を行う際に、エイリアス名を持つ Expr を生成する関数。
/// 引数に元の式を与える。その式がPrimary式ではない場合は、エイリアス名を生成できないので、None を返す。
fn create_alias(lhs: &Expr) -> Option<Expr> {
    // 補完用に生成した式には、仮に左辺の位置情報を入れておく
    let loc = lhs.loc();

    match lhs {
        Expr::Primary(prim) if prim.is_identifier() => {
            // Primary式であり、さらに識別子である場合のみ、エイリアス名を作成する
            let element = prim.element();
            element
                .split('.')
                .last()
                .map(|s| Expr::Primary(Box::new(PrimaryExpr::new(convert_identifier_case(s), loc))))
        }
        _ => None,
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
) -> Result<Clause, UroboroSQLFmtError> {
    ensure_kind(cursor, keyword)?;
    let mut clause = Clause::from_node(cursor.node(), src);

    for _ in 1..keyword.split('_').count() {
        cursor.goto_next_sibling();
        ensure_kind(cursor, keyword)?;
        clause.extend_kw(cursor.node(), src);
    }

    Ok(clause)
}

/// cursorからエラー情報を生成する関数
///
/// node_kind: {nodeの種類}
/// original_token: {元のソースコードのトークン}
/// location: {nodeの位置情報}
fn create_error_info(cursor: &TreeCursor, src: &str) -> String {
    let original_token = cursor
        .node()
        .utf8_text(src.as_bytes())
        .unwrap_or("<unknown token>");

    format!(
        "node_kind: {}\noriginal_token: {}\nlocation: {:#?}",
        cursor.node().kind(),
        original_token,
        cursor.node().range(),
    )
}
