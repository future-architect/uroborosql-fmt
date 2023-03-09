use tree_sitter::TreeCursor;

use crate::{cst::*, util::format_keyword};

use super::{create_clause, ensure_kind, Formatter, COMMENT};

impl Formatter {
    /// SELECT文
    /// 呼び出し後、cursorはselect_statementを指す
    pub(crate) fn format_select_stmt(
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
                "join_clause" => {
                    let clauses = self.format_join_cluase(cursor, src)?;
                    clauses.into_iter().for_each(|c| statement.add_clause(c));
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
                "group_by_clause" => {
                    let clauses = self.format_group_by_clause(cursor, src)?;
                    clauses.into_iter().for_each(|c| statement.add_clause(c));
                }
                "order_by_clause" => {
                    let clause = self.format_order_by_clause(cursor, src)?;
                    statement.add_clause(clause);
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

    /// DELETE文をStatement構造体で返す
    pub(crate) fn format_delete_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new(self.state.depth);

        cursor.goto_first_child();

        // DELETE
        let mut clause = create_clause(cursor, src, "DELETE", self.state.depth)?;
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
                    let clause =
                        self.format_simple_clause(cursor, src, "returning_clause", "RETURNING")?;
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
    pub(crate) fn format_update_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new(self.state.depth);
        cursor.goto_first_child();

        let mut update_clause = create_clause(cursor, src, "UPDATE", self.state.depth)?;
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
                    let clause =
                        self.format_simple_clause(cursor, src, "returning_clause", "RETURNING")?;
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

    /// INSERT文をStatementで返す
    pub(crate) fn format_insert_stmt(
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

        let mut insert = create_clause(cursor, src, "INSERT", self.state.depth)?;
        cursor.goto_next_sibling();
        // SQL_IDがあるかをチェック
        self.consume_sql_id(cursor, src, &mut insert);
        self.consume_comment_in_clause(cursor, src, &mut insert)?;

        statement.add_clause(insert);

        let mut clause = create_clause(cursor, src, "INTO", self.state.depth)?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

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
        clause.set_body(Body::Insert(Box::new(insert_body)));
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
            let returning =
                self.format_simple_clause(cursor, src, "returning_clause", "RETURNING")?;
            statement.add_clause(returning);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        ensure_kind(cursor, "insert_statement")?;

        Ok(statement)
    }
}
