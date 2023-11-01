use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT},
};

impl Visitor {
    /// INSERT文をStatementで返す
    pub(crate) fn visit_insert_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new();
        let loc = Location::new(cursor.node().range());

        // コーディング規約では、INSERTとINTOの間に改行がある
        // そのため、INSERTがキーワードの句をキーワードのみ(SQL_IDはこちらに含む)のClauseとして定義し、
        // 本体をINTOがキーワードであるClauseに追加することで実現する

        cursor.goto_first_child();
        // cusor -> with_clause?

        if cursor.node().kind() == "with_clause" {
            // with句を追加する
            let mut with_clause = self.visit_with_clause(cursor, src)?;
            cursor.goto_next_sibling();
            // with句の後に続くコメントを消費する
            self.consume_comment_in_clause(cursor, src, &mut with_clause)?;

            statement.add_clause(with_clause);
        }

        // cursor -> INSERT
        ensure_kind(cursor, "INSERT", src)?;

        let mut insert = create_clause(cursor, src, "INSERT")?;
        cursor.goto_next_sibling();
        // SQL_IDがあるかをチェック
        self.consume_or_complement_sql_id(cursor, src, &mut insert);
        self.consume_comment_in_clause(cursor, src, &mut insert)?;

        statement.add_clause(insert);

        let mut clause = create_clause(cursor, src, "INTO")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> table_name

        // table_nameは_aliasable_identifierであるが、CST上では_aliasable_expressionと等しいため、
        // visit_aliasable_exprを使用する
        let table_name = self.visit_aliasable_expr(cursor, src, None)?;
        let mut insert_body = InsertBody::new(loc, table_name);

        cursor.goto_next_sibling();
        // table_name直後のコメントを処理する
        if cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            insert_body.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        let mut is_first_content = true;

        // column_name
        if cursor.node().kind() == "(" {
            let mut sep_lines = SeparatedLines::new();
            while cursor.goto_next_sibling() {
                match cursor.node().kind() {
                    "identifier" | "dotted_name" => {
                        // 最初の式はコンマなしで式を追加する
                        if is_first_content {
                            sep_lines.add_expr(
                                self.visit_expr(cursor, src)?.to_aligned(),
                                None,
                                vec![],
                            );
                            is_first_content = false;
                        } else {
                            sep_lines.add_expr(
                                self.visit_expr(cursor, src)?.to_aligned(),
                                Some(COMMA.to_string()),
                                vec![],
                            );
                        }
                    }
                    ")" => {
                        insert_body.set_column_name(sep_lines);
                        break;
                    }
                    COMMENT => {
                        let comment = Comment::new(cursor.node(), src);
                        sep_lines.add_comment_to_child(comment)?;
                    }
                    "ERROR" => {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_insert_stmt: ERROR node appeared \n{}",
                            error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    _ => continue,
                }
            }
        }

        cursor.goto_next_sibling();

        // {VALUES ( { expression | DEFAULT } [, ...] ) [, ...] | query }
        match cursor.node().kind() {
            "values_clause" => {
                cursor.goto_first_child();
                ensure_kind(cursor, "VALUES", src)?;

                let mut items = vec![];
                // commaSep1(values_clause_item)
                while cursor.goto_next_sibling() {
                    match cursor.node().kind() {
                        "values_clause_item" => {
                            items.push(self.visit_values_clause_item(cursor, src)?);
                        }
                        COMMA => continue,
                        _ => {
                            return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                                "visit_insert_stmt(): unexpected token {}\n{}",
                                cursor.node().kind(),
                                error_annotation_from_cursor(cursor, src)
                            )))
                        }
                    }
                }

                if items.len() == 1 {
                    // カラムリストが一つのみであるとき、複数行で描画する
                    items
                        .iter_mut()
                        .for_each(|col_list| col_list.set_force_multi_line(true));
                }
                insert_body.set_values_clause(&convert_keyword_case("VALUES"), items);

                cursor.goto_parent();
                ensure_kind(cursor, "values_clause", src)?;

                cursor.goto_next_sibling();
            }
            "select_statement" => {
                // select文
                let stmt = self.visit_select_stmt(cursor, src)?;

                insert_body.set_query(stmt);

                cursor.goto_next_sibling();
            }
            "select_subexpression" => {
                // 括弧付きSELECT
                let selct_sub = self.visit_select_subexpr(cursor, src)?;

                insert_body.set_paren_query(Expr::Sub(Box::new(selct_sub)));

                cursor.goto_next_sibling();
            }
            _ => {}
        }

        // on_conflict句
        if cursor.node().kind() == "on_conflict_clause" {
            let on_conflict = self.visit_on_conflict(cursor, src)?;
            insert_body.set_on_conflict(on_conflict);
            cursor.goto_next_sibling();
        }

        clause.set_body(Body::Insert(Box::new(insert_body)));
        statement.add_clause(clause);

        // returning句
        if cursor.node().kind() == "returning_clause" {
            let returning =
                self.visit_simple_clause(cursor, src, "returning_clause", "RETURNING")?;
            statement.add_clause(returning);
            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        ensure_kind(cursor, "insert_statement", src)?;

        Ok(statement)
    }

    /// ON CONFLICT句をOnConflict構造体で返す
    fn visit_on_conflict(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<OnConflict, UroboroSQLFmtError> {
        // on_conflict_clause =
        //      ON CONFLICT
        //      [ conflict_target ]
        //      conflict_action

        cursor.goto_first_child();

        // cursor -> "ON_CONFLICT"
        ensure_kind(cursor, "ON_CONFLICT", src)?;
        let on_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

        cursor.goto_next_sibling();
        // cursor -> "ON_CONFLICT"
        ensure_kind(cursor, "ON_CONFLICT", src)?;
        let conflict_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();
        let on_conflict_keyword = (
            convert_keyword_case(on_keyword),
            convert_keyword_case(conflict_keyword),
        );

        cursor.goto_next_sibling();

        // conflict_target =
        //      ( index_column_name  [ COLLATE collation ] [ op_class ] [, ...] ) [ WHERE index_predicate ]
        //      ON CONSTRAINT constraint_name
        let conflict_target = if cursor.node().kind() == "conflict_target" {
            let conflict_target = self.visit_conflict_target(cursor, src)?;

            cursor.goto_next_sibling();
            // cursor -> conflict_action

            Some(conflict_target)
        } else {
            None
        };

        ensure_kind(cursor, "conflict_action", src)?;

        cursor.goto_first_child();

        // conflict_action =
        //      DO NOTHING
        //      DO UPDATE SET { column_name = { expression | DEFAULT } |
        //                      ( column_name [, ...] ) = [ ROW ] ( { expression | DEFAULT } [, ...] ) |
        //                      ( column_name [, ...] ) = ( sub-SELECT )
        //                    } [, ...]
        //                [ WHERE condition ]
        let conflict_action = match cursor.node().kind() {
            "DO_NOTHING" => {
                let do_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

                cursor.goto_next_sibling();
                ensure_kind(cursor, "DO_NOTHING", src)?;

                let nothing_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

                let do_nothing_keyword = (convert_keyword_case(do_keyword), convert_keyword_case(nothing_keyword));

                ConflictAction::DoNothing(DoNothing::new(do_nothing_keyword))
            }
            "DO_UPDATE" => {
                let do_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();

                cursor.goto_next_sibling();
                ensure_kind(cursor, "DO_UPDATE", src)?;

                let update_keyword = cursor.node().utf8_text(src.as_bytes()).unwrap();
                let do_update_keyword = (convert_keyword_case(do_keyword), convert_keyword_case(update_keyword));
                cursor.goto_next_sibling();

                let set_clause = self.visit_set_clause(cursor, src)?;

                cursor.goto_next_sibling();

                let mut where_clause = None;
                if cursor.node().kind() == "where_clause"{
                    where_clause = Some(self.visit_where_clause(cursor, src)?)
                }

                ConflictAction::DoUpdate(DoUpdate::new(do_update_keyword, set_clause, where_clause))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_on_conflict: expected node is 'DO_NOTHING' or 'DO_UPDATE', but actual {}\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        ensure_kind(cursor, "conflict_action", src)?;

        cursor.goto_parent();
        ensure_kind(cursor, "on_conflict_clause", src)?;

        let on_conflict = OnConflict::new(on_conflict_keyword, conflict_target, conflict_action);

        Ok(on_conflict)
    }

    /// values_clause_itemを処理する。
    /// ColumnList構造体で結果を返す。
    fn visit_values_clause_item(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        cursor.goto_first_child();
        let column_list = self.visit_column_list(cursor, src)?;
        cursor.goto_parent();
        ensure_kind(cursor, "values_clause_item", src)?;

        Ok(column_list)
    }
}
