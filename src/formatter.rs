use tree_sitter::{Node, TreeCursor};

pub(crate) const COMMENT: &str = "comment";

use crate::cst::*;
use crate::util::*;

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

    /// SELECT文
    /// 呼び出し後、cursorはselect_statementを指す
    fn format_select_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // SELECT文の定義
        // select_statement =
        //      select_clause
        //      [from_clause]
        //      [where_clause]
        //      [_combining_query]

        let mut statement = Statement::new(self.state.depth);

        // select_statementは必ずselect_clauseを子供に持つ
        cursor.goto_first_child();

        // cursor -> select_clause

        // select句を追加する
        statement.add_clause(self.format_select_clause(cursor, src)?);

        // from句以下を追加する
        while cursor.goto_next_sibling() {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            match cursor.node().kind() {
                "from_clause" => {
                    let clause = self.format_from_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    let clause = self.format_where_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                "UNION" | "INTERSECT" | "EXCEPT" => {
                    // 演算(e.g., "INTERSECT", "UNION ALL", ...)
                    let mut combining_clause = Clause::new(cursor.node(), src, self.state.depth);

                    cursor.goto_next_sibling();
                    // cursor -> (ALL | DISTINCT) | select_statement

                    if matches!(cursor.node().kind(), "ALL" | "DISTINCT") {
                        // ALL または DISTINCT を追加する
                        combining_clause.extend_kw(cursor.node(), src);
                        cursor.goto_next_sibling();
                    }
                    // cursor -> comments | select_statement

                    // 演算子のみからなる句を追加
                    statement.add_clause(combining_clause);

                    while cursor.node().kind() == COMMENT {
                        let comment = Comment::new(cursor.node(), src);
                        statement.add_comment_to_child(comment)?;
                        cursor.goto_next_sibling();
                    }

                    // 副問い合わせを計算
                    let select_stmt = self.format_select_stmt(cursor, src)?;
                    select_stmt
                        .get_clauses()
                        .iter()
                        .for_each(|clause| statement.add_clause(clause.to_owned()));

                    // cursorはselect_statementになっているはずである
                }
                COMMENT => {
                    statement.add_comment_to_child(Comment::new(cursor.node(), src))?;
                }
                _ => {
                    break;
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "select_statement")?;

        Ok(statement)
    }

    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    fn format_select_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // SELECT句の定義
        //    select_clause =
        //        "SELECT"
        //        select_clause_body

        // select_clauseは必ずSELECTを子供に持っているはずである
        cursor.goto_first_child();

        // cursor -> SELECT
        ensure_kind(cursor, "SELECT")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);

        cursor.goto_next_sibling();
        // cursor -> comments | select_clause_body

        self.consume_sql_id(cursor, src, &mut clause);
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> select_caluse_body

        let body = self.format_select_clause_body(cursor, src)?;
        clause.set_body(body);

        // cursorをselect_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause")?;

        Ok(clause)
    }

    /// SELECT句の本体をSeparatedLinesで返す
    /// 呼び出し後、cursorはselect_clause_bodyを指している
    fn format_select_clause_body(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // select_clause_bodyは必ず_aliasable_expressionを子供に持つ
        cursor.goto_first_child();

        // cursor -> _aliasable_expression
        // commaSep1(_aliasable_expression)
        let body = self.format_comma_sep_alias(cursor, src, false)?;

        // cursorをselect_clause_bodyに
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause_body")?;

        Ok(body)
    }

    /// FROM句をClause構造体で返す
    fn format_from_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clauseは必ずFROMを子供に持つ
        cursor.goto_first_child();

        // cursor -> FROM
        ensure_kind(cursor, "FROM")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);

        cursor.goto_next_sibling();
        // cursor -> comments | _aliasable_expression

        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> aliasable_expression
        // commaSep1(_aliasable_expression)
        let body = self.format_comma_sep_alias(cursor, src, true)?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause")?;

        Ok(clause)
    }

    fn format_where_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // where_clauseは必ずWHEREを子供に持つ
        cursor.goto_first_child();

        // cursor -> WHERE
        ensure_kind(cursor, "WHERE")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);

        cursor.goto_next_sibling();
        // cursor -> COMMENT | _expression

        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> _expression
        let expr = self.format_expr(cursor, src)?;

        // 結果として得られた式をBodyに変換する
        let body = Body::with_expr(expr, self.state.depth);

        clause.set_body(body);

        // cursorをwhere_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "where_clause")?;

        Ok(clause)
    }

    /// DELETE文をStatement構造体で返す
    fn format_delete_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new(self.state.depth);

        cursor.goto_first_child();
        // DELETE
        ensure_kind(cursor, "DELETE")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        self.consume_sql_id(cursor, src, &mut clause);
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        statement.add_clause(clause);

        // cursor -> from_clause
        let from_clause = self.format_from_clause(cursor, src)?;
        statement.add_clause(from_clause);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "where_clause" => {
                    let clause = self.format_where_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                "returning_clause" => {
                    let clause = self.format_returning_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_delete_stmt(): unimplemented delete_statement\nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )))
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "delete_statement")?;

        Ok(statement)
    }

    /// UPDATE文をStatement構造体で返す
    fn format_update_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new(self.state.depth);
        cursor.goto_first_child();

        // キーワードの確認
        ensure_kind(cursor, "UPDATE")?;
        let mut update_clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        self.consume_sql_id(cursor, src, &mut update_clause);
        self.consume_comment_in_clause(cursor, src, &mut update_clause)?;

        // 規則上でここに現れるノードは_aliasable_identifierだが、'_'から始まっているためノードに現れない。
        // _expression、_aliasable_expressionもノードに現れないため、
        // _aliasable_identifierは実質的に_aliasable_expressionと同じCSTになっている
        let table_name = self.format_aliasable_expr(cursor, src)?;

        // update句を追加する
        // update句のエイリアスはASを省略するため、第三引数のis_omit_opをtrueにしてSeparatedLinesを生成する
        let mut sep_lines = SeparatedLines::new(self.state.depth, ",", true);
        sep_lines.add_expr(table_name);
        update_clause.set_body(Body::SepLines(sep_lines));
        statement.add_clause(update_clause);

        cursor.goto_next_sibling();

        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // set句を処理する
        ensure_kind(cursor, "set_clause")?;
        let set_clause = self.format_set_clause(cursor, src)?;
        statement.add_clause(set_clause);

        // where句、returning句を持つ可能性がある
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                "where_clause" => {
                    let clause = self.format_where_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                "returning_clause" => {
                    let clause = self.format_returning_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_update_stmt(): unimplemented update_statement\nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )))
                }
            }
        }

        cursor.goto_parent();
        ensure_kind(cursor, "update_statement")?;

        Ok(statement)
    }

    /// SET句をClause構造体で返す
    /// UPDATE文、INSERT文で使用する
    fn format_set_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "SET")?;
        let mut set_clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        ensure_kind(cursor, "set_clause_body")?;
        cursor.goto_first_child();

        let mut sep_lines = SeparatedLines::new(self.state.depth, ",", false);

        let mut format_set_clause_item = |cursor: &mut TreeCursor| {
            if cursor.node().kind() == "assigment_expression" {
                // tree-sitter-sqlのタイポでnが抜けている点に注意
                let aligned = self.format_assign_expr(cursor, src)?;
                Ok(aligned)
            } else if cursor.node().kind() == "(" {
                let lhs = Expr::ColumnList(Box::new(self.format_column_list(cursor, src)?));
                cursor.goto_next_sibling();
                ensure_kind(cursor, "=")?;

                cursor.goto_next_sibling();

                let rhs = if cursor.node().kind() == "select_subexpression" {
                    self.nest();
                    let expr = Expr::SelectSub(Box::new(self.format_select_subexpr(cursor, src)?));
                    self.unnest();
                    expr
                } else {
                    Expr::ColumnList(Box::new(self.format_column_list(cursor, src)?))
                };

                let mut aligned = AlignedExpr::new(lhs, false);
                aligned.add_rhs("=".to_string(), rhs);

                Ok(aligned)
            } else {
                Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                    r#"format_set_clause(): expected node is assigment_expression, "(" or select_subexpression, but actual {}\n{:#?}"#,
                    cursor.node().kind(),
                    cursor.node().range()
                )))
            }
        };

        // commaSep1(set_clause_item)
        let aligned = format_set_clause_item(cursor)?;
        sep_lines.add_expr(aligned);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);
                    sep_lines.add_comment_to_child(comment)?;
                }
                "," => continue,
                _ => {
                    let aligned = format_set_clause_item(cursor)?;
                    sep_lines.add_expr(aligned);
                }
            }
        }
        cursor.goto_parent();
        ensure_kind(cursor, "set_clause_body")?;

        // set_clauseにBodyをセット
        set_clause.set_body(Body::SepLines(sep_lines));

        cursor.goto_parent();
        ensure_kind(cursor, "set_clause")?;

        Ok(set_clause)
    }

    fn format_assign_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let identifier = self.format_expr(cursor, src)?;
        cursor.goto_next_sibling();
        ensure_kind(cursor, "=")?;
        cursor.goto_next_sibling();
        let expr = self.format_expr(cursor, src)?;

        let mut aligned = AlignedExpr::new(identifier, false);
        aligned.add_rhs("=".to_string(), expr);
        cursor.goto_parent();
        ensure_kind(cursor, "assigment_expression")?;

        Ok(aligned)
    }

    /// INSERT文をStatementで返す
    fn format_insert_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new(self.state.depth);
        let loc = Location::new(cursor.node().range());

        // コーディング規約では、INSERTとINTOの間に改行がある
        // そのため、INSERTがキーワードの句をキーワードのみ(SQL_IDはこちらに含む)のClauseとして定義し、
        // 本体をINTOがキーワードであるClauseに追加することで実現する

        cursor.goto_first_child();
        ensure_kind(cursor, "INSERT")?;
        let mut insert = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        // SQL_IDがあるかをチェック
        self.consume_sql_id(cursor, src, &mut insert);

        statement.add_clause(insert);

        ensure_kind(cursor, "INTO")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);

        cursor.goto_next_sibling();
        // cursor -> table_name

        // table_nameは_aliasable_identifierであるが、CST上では_aliasable_expressionと等しいため、
        // format_aliasable_exprを使用する
        let table_name = self.format_aliasable_expr(cursor, src)?;
        let mut insert_body = InsertBody::new(self.state.depth, loc, table_name);

        cursor.goto_next_sibling();
        // table_name直後のコメントを処理する
        if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            insert_body.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // column_name
        if cursor.node().kind() == "(" {
            let mut sep_lines = SeparatedLines::new(self.state.depth, ",", false);
            while cursor.goto_next_sibling() {
                match cursor.node().kind() {
                    "identifier" | "dotted_name" => {
                        sep_lines.add_expr(self.format_aliasable_expr(cursor, src)?);
                    }
                    ")" => {
                        insert_body.set_column_name(sep_lines);
                        break;
                    }
                    COMMENT => {
                        let comment = Comment::new(cursor.node(), src);
                        sep_lines.add_comment_to_child(comment)?;
                    }
                    _ => continue,
                }
            }
        }

        // values_clause_itemを処理するクロージャ
        // ColumnList構造体で結果を返す
        let mut format_values_clause_item =
            |cursor: &mut TreeCursor| -> Result<ColumnList, UroboroSQLFmtError> {
                cursor.goto_first_child();
                let column_list = self.format_column_list(cursor, src)?;
                cursor.goto_parent();
                ensure_kind(cursor, "values_clause_item")?;

                Ok(column_list)
            };

        cursor.goto_next_sibling();

        // values句
        if cursor.node().kind() == "values_clause" {
            cursor.goto_first_child();
            ensure_kind(cursor, "VALUES")?;

            let mut items = vec![];
            // commaSep1(values_clause_item)
            while cursor.goto_next_sibling() {
                match cursor.node().kind() {
                    "values_clause_item" => {
                        items.push(format_values_clause_item(cursor)?);
                    }
                    "," => continue,
                    _ => {
                        return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                            "format_insert_stmt(): unexpected token {}\n{:#?}",
                            cursor.node().kind(),
                            cursor.node().range()
                        )))
                    }
                }
            }
            insert_body.set_values_clause(&format_keyword("VALUES"), items);

            cursor.goto_parent();
            ensure_kind(cursor, "values_clause")?;

            cursor.goto_next_sibling();
        }

        // InsertBodyに含めるのは、テーブル名、カラム名、VALUES句である
        // そのため、ここでstatementに追加する
        clause.set_body(Body::Insert(insert_body));
        statement.add_clause(clause);

        // select文
        if cursor.node().kind() == "select_statement" {
            let stmt = self.format_select_stmt(cursor, src)?;
            for clause in stmt.get_clauses() {
                statement.add_clause(clause);
            }
            cursor.goto_next_sibling();
        }

        // returning句
        if cursor.node().kind() == "returning_clause" {
            let returning = self.format_returning_clause(cursor, src)?;
            statement.add_clause(returning);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        ensure_kind(cursor, "insert_statement")?;

        Ok(statement)
    }

    /// RETURNING句をClauseで返す
    fn format_returning_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        ensure_kind(cursor, "RETURNING")?;
        let mut clause = Clause::new(cursor.node(), src, self.state.depth);
        cursor.goto_next_sibling();

        let body = self.format_comma_sep_alias(cursor, src, false)?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "returning_clause")?;

        Ok(clause)
    }

    /// カラムリストをColumnListで返す
    /// カラムリストはVALUES句、SET句で現れ、"(" 式 ["," 式 ...] ")"という構造になっている
    fn format_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        ensure_kind(cursor, "(")?;
        let mut loc = Location::new(cursor.node().range());

        let mut exprs = vec![];
        // commaSep1(_expression)
        while cursor.goto_next_sibling() {
            loc.append(Location::new(cursor.node().range()));
            match cursor.node().kind() {
                "," => continue,
                ")" => break,
                COMMENT => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_column_list(): Unexpected comment\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )))
                }
                _ => {
                    exprs.push(self.format_expr(cursor, src)?);
                }
            }
        }

        Ok(ColumnList::new(exprs, loc))
    }

    /// エイリアス可能な式
    /// 呼び出し後、cursorはaliasまたは式のノードを指している
    fn format_aliasable_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // エイリアス可能な式の定義
        //    _aliasable_expression =
        //        alias | _expression

        //    alias =
        //        _expression
        //        ["AS"]
        //        identifier

        match cursor.node().kind() {
            "alias" => {
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let lhs_expr = self.format_expr(cursor, src)?;

                let mut aligned = AlignedExpr::new(lhs_expr, true);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> trailing_comment | "AS"?

                    if cursor.node().kind() == COMMENT {
                        // ASの直前にcommentがある場合
                        let comment = Comment::new(cursor.node(), src);

                        if comment.is_multi_line_comment()
                            || !comment.loc().is_same_line(&aligned.loc())
                        {
                            // 行末以外のコメント(次以降の行のコメント)は未定義
                            // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                            // エイリアスがない場合は、コメントノードがここに現れない
                            return Err(UroboroSQLFmtError::UnexpectedSyntaxError(format!(
                                "format_aliasable_expr(): unexpected syntax\nnode_kind: {}\n{:#?}",
                                cursor.node().kind(),
                                cursor.node().range(),
                            )));
                        } else {
                            // 行末コメント
                            aligned.set_lhs_trailing_comment(comment)?;
                        }
                        cursor.goto_next_sibling();
                    }

                    // ASが存在する場合は読み飛ばす
                    if cursor.node().kind() == "AS" {
                        cursor.goto_next_sibling();
                    }

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    if cursor.node().kind() == "identifier" {
                        let rhs = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        let rhs_expr =
                            PrimaryExpr::new(rhs.to_string(), Location::new(cursor.node().range()));
                        aligned.add_rhs(format_keyword("AS"), Expr::Primary(Box::new(rhs_expr)));
                    }
                }

                // cursorをalias に戻す
                cursor.goto_parent();

                Ok(aligned)
            }
            _ => {
                // _expression
                let expr = self.format_expr(cursor, src)?;

                Ok(AlignedExpr::new(expr, true))
            }
        }
    }

    /// 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        matches!(
            op_str,
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
        )
    }

    /// 式のフォーマットを行う。
    /// cursorがコメントを指している場合、バインドパラメータであれば結合して返す。
    /// 式の初めにバインドパラメータが現れた場合、式の本体は隣の兄弟ノードになる。
    /// 呼び出し後、cursorは式の本体のノードを指す
    fn format_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // バインドパラメータをチェック
        let head_comment = if cursor.node().kind() == COMMENT {
            let comment_node = cursor.node();
            cursor.goto_next_sibling();
            // cursor -> _expression
            // 式の直前に複数コメントが来る場合は想定していない
            Some(Comment::new(comment_node, src))
        } else {
            None
        };

        let mut result = match cursor.node().kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                // cursor -> dotted_name

                let range = cursor.node().range();

                cursor.goto_first_child();
                // cursor -> identifier

                let mut dotted_name = String::new();

                let id_node = cursor.node();
                dotted_name.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                while cursor.goto_next_sibling() {
                    // cursor -> . または cursor -> identifier
                    match cursor.node().kind() {
                        "." => dotted_name.push('.'),
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(dotted_name, Location::new(range));

                // cursorをdotted_nameに戻す
                cursor.goto_parent();
                ensure_kind(cursor, "dotted_name")?;

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                // cursor -> binary_expression

                cursor.goto_first_child();
                // cursor -> _expression

                // 左辺
                let lhs_expr = self.format_expr(cursor, src)?;

                cursor.goto_next_sibling();
                // cursor -> op (e.g., "+", "-", "=", ...)

                // 演算子
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                cursor.goto_next_sibling();
                // cursor -> _expression

                // 右辺
                let rhs_expr = self.format_expr(cursor, src)?;

                // cursorを戻しておく
                cursor.goto_parent();
                ensure_kind(cursor, "binary_expression")?;

                if Self::is_comp_op(op_str) {
                    // 比較演算子ならばそろえる必要があるため、AlignedExprとする
                    let mut aligned = AlignedExpr::new(lhs_expr, false);
                    aligned.add_rhs(op_str.to_string(), rhs_expr);

                    Expr::Aligned(Box::new(aligned))
                } else {
                    // 比較演算子でないならば、PrimaryExprに
                    // e.g.,) 1 + 1
                    match lhs_expr {
                        Expr::Primary(mut lhs) => {
                            lhs.add_element(op_str);
                            match rhs_expr {
                                Expr::Primary(rhs) => lhs.append(*rhs),
                                _ => {
                                    // 右辺が複数行の場合
                                    // todo
                                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                                        "format_expr(): (binary expression) right has multiple lines \nnode_kind: {}\n{:#?}",
                                        cursor.node().kind(),
                                        cursor.node().range(),
                                    )));
                                }
                            }
                            Expr::Primary(lhs)
                        }
                        _ => {
                            // 左辺が複数行の場合
                            // todo
                            return Err(UroboroSQLFmtError::UnimplementedError(format!(
                                "format_expr(): (binary expression) left has multiple lines \nnode_kind: {}\n{:#?}",
                                cursor.node().kind(),
                                cursor.node().range(),
                            )));
                        }
                    }
                }
            }
            "between_and_expression" => {
                Expr::Aligned(Box::new(self.format_between_and_expression(cursor, src)?))
            }
            "boolean_expression" => self.format_bool_expr(cursor, src)?,
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let primary = PrimaryExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(cursor.node().range()),
                );

                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(cursor, src)?;
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.format_paren_expr(cursor, src)?;
                Expr::ParenExpr(Box::new(paren_expr))
            }
            "asterisk_expression" => {
                let asterisk = AsteriskExpr::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(cursor.node().range()),
                );
                Expr::Asterisk(Box::new(asterisk))
            }
            "conditional_expression" => {
                let cond_expr = self.format_cond_expr(cursor, src)?;
                Expr::Cond(Box::new(cond_expr))
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_expr(): unimplemented expression \nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        };

        // バインドパラメータの追加
        if let Some(comment) = head_comment {
            if comment.is_multi_line_comment() && comment.loc().is_next_to(&result.loc()) {
                // 複数行コメントかつ式に隣接していれば、バインドパラメータ
                result.set_head_comment(comment);
            } else {
                // TODO: 隣接していないコメント
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "format_expr(): (bind parameter) separated comment\nnode_kind: {}\n{:#?}",
                    cursor.node().kind(),
                    cursor.node().range(),
                )));
            }
        }

        Ok(result)
    }

    /// bool式をフォーマットする
    /// 呼び出し後、cursorはboolean_expressionを指している
    fn format_bool_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        let mut boolean_expr = BooleanExpr::new(self.state.depth, "-");

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            let mut loc = Location::new(cursor.node().range());
            cursor.goto_next_sibling();
            // cursor -> _expr

            // ここにバインドパラメータ以外のコメントは来ないことを想定している。
            let expr = self.format_expr(cursor, src)?;

            // (NOT expr)のソースコード上の位置を計算
            loc.append(expr.loc());

            let not_expr = UnaryExpr::new(&format_keyword("NOT"), expr, loc);

            cursor.goto_parent();
            ensure_kind(cursor, "boolean_expression")?;

            // Unaryとして返す
            return Ok(Expr::Unary(Box::new(not_expr)));
        } else {
            // and or
            let left = self.format_expr(cursor, src)?;

            boolean_expr.add_expr(left);

            cursor.goto_next_sibling();
            // cursor -> COMMENT | op

            while cursor.node().kind() == COMMENT {
                boolean_expr.add_comment_to_child(Comment::new(cursor.node(), src))?;
                cursor.goto_next_sibling();
            }

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(format_keyword(sep));

            cursor.goto_next_sibling();
            // cursor -> _expression

            let right = self.format_expr(cursor, src)?;

            // 左辺と同様の処理を行う
            boolean_expr.add_expr(right);
        }
        // cursorをboolean_expressionに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "boolean_expression")?;

        Ok(Expr::Boolean(Box::new(boolean_expr)))
    }

    /// かっこで囲まれたSELECTサブクエリをフォーマットする
    /// 呼び出し後、cursorはselect_subexpressionを指している
    fn format_select_subexpr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SelectSubExpr, UroboroSQLFmtError> {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(cursor.node().range());

        // cursor -> select_subexpression

        cursor.goto_first_child();
        // cursor -> (
        // 将来的には、かっこの数を数えるかもしれない
        self.nest();

        cursor.goto_next_sibling();
        // cursor -> comments | select_statement

        let mut comment_buf: Vec<Comment> = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_statement
        let mut select_stmt = self.format_select_stmt(cursor, src)?;

        // select_statementの前にコメントがあった場合、コメントを追加
        comment_buf
            .into_iter()
            .for_each(|c| select_stmt.add_comment(c));

        cursor.goto_next_sibling();
        // cursor -> comments | )

        while cursor.node().kind() == COMMENT {
            // 閉じかっこの直前にコメントが来る場合
            let comment = Comment::new(cursor.node(), src);
            select_stmt.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> )
        self.unnest();

        cursor.goto_parent();
        ensure_kind(cursor, "select_subexpression")?;

        Ok(SelectSubExpr::new(select_stmt, loc, self.state.depth))
    }

    /// かっこで囲まれた式をフォーマットする
    /// 呼び出し後、cursorはparenthesized_expressionを指す
    fn format_paren_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenExpr, UroboroSQLFmtError> {
        // parenthesized_expression: $ => PREC.unary "(" expression ")"
        // TODO: cursorを引数で渡すよう変更したことにより、tree-sitter-sqlの規則を
        //       _parenthesized_expressionに戻してもよくなったため、修正する

        let loc = Location::new(cursor.node().range());

        // 括弧の前の演算子には未対応

        cursor.goto_first_child();
        // cursor -> "("

        cursor.goto_next_sibling();
        // cursor -> comments | expr

        let mut comment_buf = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr

        // exprがparen_exprならネストしない
        let is_nest = !matches!(cursor.node().kind(), "parenthesized_expression");

        if is_nest {
            self.nest();
        }

        let expr = self.format_expr(cursor, src)?;

        let mut paren_expr = match expr {
            Expr::ParenExpr(mut paren_expr) => {
                paren_expr.set_loc(loc);
                *paren_expr
            }
            _ => {
                let paren_expr = ParenExpr::new(expr, loc, self.state.depth);
                self.unnest();
                paren_expr
            }
        };

        // 開きかっこと式の間にあるコメントを追加
        for comment in comment_buf {
            paren_expr.add_start_comment(comment);
        }

        // かっこの中の式の最初がバインドパラメータを含む場合でも、comment_bufに読み込まれてしまう
        // そのため、現状ではこの位置のバインドパラメータを考慮していない
        cursor.goto_next_sibling();
        // cursor -> comments | ")"

        // 閉じかっこの前にあるコメントを追加
        while cursor.node().kind() == COMMENT {
            paren_expr.add_comment_to_child(Comment::new(cursor.node(), src))?;
            cursor.goto_next_sibling();
        }

        // tree-sitter-sqlを修正したら削除する
        cursor.goto_parent();
        ensure_kind(cursor, "parenthesized_expression")?;

        Ok(paren_expr)
    }

    /// CASE式をフォーマットする
    /// 呼び出し後、cursorはconditional_expressionを指す
    fn format_cond_expr(
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
                    let mut when_clause = Clause::new(cursor.node(), src, self.state.depth);

                    cursor.goto_next_sibling();
                    // cursor -> comment | _expression

                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> _expression

                    let when_expr = self.format_expr(cursor, src)?;
                    when_clause.set_body(Body::with_expr(when_expr, self.state.depth));

                    cursor.goto_next_sibling();
                    // cursor -> comment || "THEN"

                    self.consume_comment_in_clause(cursor, src, &mut when_clause)?;

                    // cursor -> "THEN"
                    ensure_kind(cursor, "THEN")?;
                    let mut then_clause = Clause::new(cursor.node(), src, self.state.depth);

                    cursor.goto_next_sibling();
                    // cursor -> comment || _expression

                    self.consume_comment_in_clause(cursor, src, &mut then_clause)?;

                    // cursor -> _expression

                    let then_expr = self.format_expr(cursor, src)?;
                    then_clause.set_body(Body::with_expr(then_expr, self.state.depth));

                    cond_expr.add_when_then_clause(when_clause, then_clause);
                }
                "ELSE" => {
                    let mut else_clause = Clause::new(cursor.node(), src, self.state.depth);

                    cursor.goto_next_sibling();
                    // cursor -> comment || _expression

                    self.consume_comment_in_clause(cursor, src, &mut else_clause)?;

                    // cursor -> _expression

                    let else_expr = self.format_expr(cursor, src)?;
                    else_clause.set_body(Body::with_expr(else_expr, self.state.depth));

                    cond_expr.set_else_clause(else_clause);
                }
                "END" => {
                    break;
                }
                "comment" => {
                    let comment_node = cursor.node();
                    let comment = Comment::new(comment_node, src);

                    // 行末コメントを式にセットする
                    cond_expr.set_trailing_comment(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnimplementedError(format!(
                        "format_cond_expr(): unimplemented conditional_expression\nnode_kind: {}\n{:#?}",
                        cursor.node().kind(),
                        cursor.node().range(),
                    )))
                } // error
            }
        }

        self.unnest();
        self.unnest();

        cursor.goto_parent();
        ensure_kind(cursor, "conditional_expression")?;

        Ok(cond_expr)
    }

    /// BETWEEN述語をフォーマットする
    /// 呼び出し後、cursorはbetween_and_expressionを指す
    fn format_between_and_expression(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // between_and_expressionに子供がいないことはない
        cursor.goto_first_child();
        // cursor -> expression

        let expr = self.format_expr(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> (NOT)? BETWEEN

        let mut operator = String::new();

        if cursor.node().kind() == "NOT" {
            operator += &format_keyword("NOT");
            operator += " "; // betweenの前に空白を入れる
            cursor.goto_next_sibling();
        }

        ensure_kind(cursor, "BETWEEN")?;
        operator += &format_keyword("BETWEEN");
        cursor.goto_next_sibling();
        // cursor -> _expression

        let from_expr = self.format_expr(cursor, src)?;
        cursor.goto_next_sibling();
        // cursor -> AND

        ensure_kind(cursor, "AND")?;
        cursor.goto_next_sibling();
        // cursor -> _expression

        let to_expr = self.format_expr(cursor, src)?;

        // (from AND to)をAlignedExprにまとめる
        let mut rhs = AlignedExpr::new(from_expr, false);
        rhs.add_rhs(format_keyword("AND"), to_expr);

        // (expr BETWEEN rhs)をAlignedExprにまとめる
        let mut aligned = AlignedExpr::new(expr, false);
        aligned.add_rhs(operator, Expr::Aligned(Box::new(rhs)));

        cursor.goto_parent();
        ensure_kind(cursor, "between_and_expression")?;

        Ok(aligned)
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
                "," => continue,
                COMMENT => {
                    separated_lines.add_comment_to_child(Comment::new(cursor.node(), src))?;
                }
                _ => {
                    // _aliasable_expression
                    let alias = self.format_aliasable_expr(cursor, src)?;
                    separated_lines.add_expr(alias);
                }
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
