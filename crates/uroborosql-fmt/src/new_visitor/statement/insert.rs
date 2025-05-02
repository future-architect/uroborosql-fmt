use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        pg_insert::PgInsertBody, AlignedExpr, Body, Collate, Comment, ConflictAction,
        ConflictTarget, ConflictTargetColumnList, ConflictTargetElement, DoNothing, DoUpdate, Expr,
        Location, OnConflict, OnConstraint, PrimaryExpr, SeparatedLines, SpecifyIndexColumn,
        Statement,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, COMMA},
    util::{convert_identifier_case, convert_keyword_case},
    NewVisitor as Visitor,
};

// InsertStmt:
// - opt_with_clause INSERT INTO insert_target insert_rest? opt_on_conflict? returning_clause?
//
// insert_target:
// - qualified_name
// - qualified_name AS ColId
//
// insert_rest:
// - SelectStmt
// - OVERRIDING override_kind VALUE_P SelectStmt
// - '(' insert_column_list ')' SelectStmt
// - '(' insert_column_list ')' OVERRIDING override_kind VALUE_P SelectStmt
// - DEFAULT VALUES
//
// opt_on_conflict:
// - ON CONFLICT opt_conf_expr? DO UPDATE SET set_clause_list where_clause
// - ON CONFLICT opt_conf_expr? DO NOTHING

impl Visitor {
    pub(crate) fn visit_insert_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // InsertStmt:
        // - opt_with_clause INSERT INTO insert_target insert_rest? opt_on_conflict? returning_clause?

        let mut statement = Statement::new();
        let loc = Location::from(cursor.node().range());

        cursor.goto_first_child();
        // cursor -> opt_with_clause?

        if cursor.node().kind() == SyntaxKind::opt_with_clause {
            // opt_with_clause
            // - with_clause

            cursor.goto_first_child();
            pg_ensure_kind!(cursor, SyntaxKind::with_clause, src);

            let with_clause = self.visit_with_clause(cursor, src)?;

            statement.add_clause(with_clause);

            cursor.goto_parent();
            pg_ensure_kind!(cursor, SyntaxKind::opt_with_clause, src);

            cursor.goto_next_sibling();
        }

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // コーディング規約では、INSERTとINTOの間に改行がある
        // そのため、INSERTがキーワードの句をキーワードのみ(SQL_IDはこちらに含む)のClauseとして定義し、
        // 本体をINTOがキーワードであるClauseに追加することで実現する

        // cursor -> INSERT
        let mut insert_keyword_clause = pg_create_clause!(cursor, SyntaxKind::INSERT);
        cursor.goto_next_sibling();
        // SQL_IDがあるかをチェック
        self.pg_consume_or_complement_sql_id(cursor, &mut insert_keyword_clause);
        self.pg_consume_comments_in_clause(cursor, &mut insert_keyword_clause)?;

        statement.add_clause(insert_keyword_clause);

        // cursor -> INTO
        let mut into_keyword_clause = pg_create_clause!(cursor, SyntaxKind::INTO);
        cursor.goto_next_sibling();
        self.pg_consume_comments_in_clause(cursor, &mut into_keyword_clause)?;

        // cursor -> insert_target
        pg_ensure_kind!(cursor, SyntaxKind::insert_target, src);
        let insert_target = self.visit_insert_target(cursor, src)?;
        let mut insert_body = PgInsertBody::new(loc, insert_target);

        cursor.goto_next_sibling();
        // テーブル名直後のコメントを処理する
        // cursor -> comment?
        if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            insert_body.add_comment_to_child(comment)?;

            cursor.goto_next_sibling();
        }

        // cursor -> insert_rest?
        if cursor.node().kind() == SyntaxKind::insert_rest {
            let (columns, query) = self.visit_insert_rest(cursor, src)?;

            if let Some(columns) = columns {
                insert_body.set_column_name(columns);
            }

            if let Some(query) = query {
                insert_body.set_query(query);
            }

            cursor.goto_next_sibling();
        }

        // cursor -> opt_on_conflict?
        if cursor.node().kind() == SyntaxKind::opt_on_conflict {
            let on_conflict = self.visit_on_conflict(cursor, src)?;
            insert_body.set_on_conflict(on_conflict);

            cursor.goto_next_sibling();
        }

        into_keyword_clause.set_body(Body::PgInsert(Box::new(insert_body)));
        statement.add_clause(into_keyword_clause);

        // cursor -> returning_clause?
        if cursor.node().kind() == SyntaxKind::returning_clause {
            let returning = self.visit_returning_clause(cursor, src)?;
            statement.add_clause(returning);

            cursor.goto_next_sibling();
        }

        cursor.goto_parent();
        // cursor -> InsertStmt
        pg_ensure_kind!(cursor, SyntaxKind::InsertStmt, src);

        Ok(statement)
    }

    fn visit_insert_target(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // insert_target:
        // - qualified_name
        // - qualified_name AS ColId

        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::qualified_name, src);

        let loc = Location::from(cursor.node().range());

        // qualified_name の子ノードは走査せず、一括でテキストを取得する
        // 空白を削除することでフォーマット処理とする
        let qualified_name_text = cursor.node().text();
        let whitespace_removed = qualified_name_text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        let primary = PrimaryExpr::new(convert_identifier_case(&whitespace_removed), loc);
        let mut aligned = Expr::Primary(Box::new(primary)).to_aligned();

        cursor.goto_next_sibling();

        if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = convert_keyword_case(cursor.node().text());

            cursor.goto_next_sibling();
            // パーサ側の reduce/shift conflict 回避の事情のため、
            // insert_target におけるエイリアスがある場合は AS が省略されることはない
            pg_ensure_kind!(cursor, SyntaxKind::ColId, src);

            let col_id = convert_identifier_case(cursor.node().text());
            let loc = Location::from(cursor.node().range());

            let rhs = Expr::Primary(Box::new(PrimaryExpr::new(col_id, loc)));
            aligned.add_rhs(Some(as_keyword), rhs);
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::insert_target, src);

        Ok(aligned)
    }

    /// カラム名指定やテーブル式を処理する
    fn visit_insert_rest(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<SeparatedLines>, Option<Statement>), UroboroSQLFmtError> {
        // insert_rest:
        // - SelectStmt
        // - '(' insert_column_list ')' SelectStmt
        // - '(' insert_column_list ')' OVERRIDING override_kind VALUE_P SelectStmt
        // - OVERRIDING override_kind VALUE_P SelectStmt
        // - DEFAULT VALUES

        cursor.goto_first_child();

        let result = match cursor.node().kind() {
            SyntaxKind::SelectStmt => {
                let select_stmt = self.visit_select_stmt(cursor, src)?;
                (None, Some(select_stmt))
            }
            SyntaxKind::LParen => {
                // - '(' insert_column_list ')' SelectStmt
                // - '(' insert_column_list ')' OVERRIDING override_kind VALUE_P SelectStmt

                // '(' insert_column_list ')' は SeparatedLines に変換し、 SelectStmt を Statement に変換する

                cursor.goto_next_sibling();
                // cursor -> insert_column_list
                let mut column_list = self.visit_insert_column_list(cursor, src)?;
                cursor.goto_next_sibling();

                // カラム指定の最後のコメントを処理する
                if cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    column_list.add_comment_to_child(comment)?;
                    cursor.goto_next_sibling();
                }

                // cursor -> ')'
                pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

                cursor.goto_next_sibling();

                // cursor -> SelectStmt | OVERRIDING
                let query = match cursor.node().kind() {
                    SyntaxKind::SelectStmt => self.visit_select_stmt(cursor, src)?,
                    SyntaxKind::OVERRIDING => {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_insert_rest: OVERRIDING is not implemented \n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    _ => {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_insert_rest: unexpected node \n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )))
                    }
                };

                (Some(column_list), Some(query))
            }
            SyntaxKind::OVERRIDING => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_insert_rest: OVERRIDING is not implemented \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::DEFAULT => {
                // unimplemented
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_insert_rest: DEFAULT is not implemented \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_insert_rest: unexpected node \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::insert_rest, src);

        Ok(result)
    }

    fn visit_insert_column_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<SeparatedLines, UroboroSQLFmtError> {
        // insert_column_list:
        // - insert_column_item (',' insert_column_item)*

        let mut sep_lines = SeparatedLines::new();

        cursor.goto_first_child();
        // cursor -> insert_column_item
        pg_ensure_kind!(cursor, SyntaxKind::insert_column_item, src);

        let column_item = self.visit_insert_column_item(cursor, src)?;
        sep_lines.add_expr(column_item, None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::insert_column_item => {
                    let column_item = self.visit_insert_column_item(cursor, src)?;
                    sep_lines.add_expr(column_item, Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_insert_column_list: unexpected node \n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::insert_column_list, src);

        Ok(sep_lines)
    }

    fn visit_insert_column_item(
        &mut self,
        cursor: &mut TreeCursor,
        _src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // 子ノードを走査せず一括でテキストを取得、空白を削除することでフォーマット処理とする
        let column_name_text = cursor.node().text();
        let whitespace_removed = column_name_text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        let loc = Location::from(cursor.node().range());
        let primary = PrimaryExpr::new(convert_identifier_case(&whitespace_removed), loc);
        let aligned = Expr::Primary(Box::new(primary)).to_aligned();

        Ok(aligned)
    }

    fn visit_on_conflict(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<OnConflict, UroboroSQLFmtError> {
        // opt_on_conflict:
        // - ON CONFLICT opt_conf_expr? DO UPDATE SET set_clause_list where_clause
        // - ON CONFLICT opt_conf_expr? DO NOTHING

        cursor.goto_first_child();

        // cursor -> ON
        pg_ensure_kind!(cursor, SyntaxKind::ON, src);
        let on_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        // cursor -> CONFLICT
        pg_ensure_kind!(cursor, SyntaxKind::CONFLICT, src);
        let conflict_keyword = convert_keyword_case(cursor.node().text());

        let on_conflict_keyword = (on_keyword, conflict_keyword);

        cursor.goto_next_sibling();

        // cursor -> opt_conf_expr?
        let conflict_target = if cursor.node().kind() == SyntaxKind::opt_conf_expr {
            let conflict_target = self.opt_conf_expr(cursor, src)?;

            cursor.goto_next_sibling();

            Some(conflict_target)
        } else {
            None
        };

        // cursor -> DO
        let conflict_action = self.handle_conflict_action_nodes(cursor, src)?;

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_on_conflict, src);

        Ok(OnConflict::new(
            on_conflict_keyword,
            conflict_target,
            conflict_action,
        ))
    }

    fn opt_conf_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTarget, UroboroSQLFmtError> {
        // opt_conf_expr:
        // - '(' index_params ')' where_clause?
        // - ON CONSTRAINT name

        cursor.goto_first_child();

        let conflict_target = match cursor.node().kind() {
            SyntaxKind::LParen => {
                // - '(' index_params ')' where_clause?

                // cursor -> index_params
                let index_params = self.handle_conflict_target_column_list_nodes(cursor, src)?;
                let mut specify_index_column = SpecifyIndexColumn::new(index_params);

                cursor.goto_next_sibling();
                // cursor -> where_clause?
                if cursor.node().kind() == SyntaxKind::where_clause {
                    let where_clause = self.pg_visit_where_clause(cursor, src)?;
                    specify_index_column.set_where_clause(where_clause);
                };

                ConflictTarget::SpecifyIndexColumn(specify_index_column)
            }
            SyntaxKind::ON => {
                // - ON CONSTRAINT name

                // cursor -> ON
                let on_keyword = cursor.node().text();

                cursor.goto_next_sibling();
                // cursor -> CONSTRAINT
                pg_ensure_kind!(cursor, SyntaxKind::CONSTRAINT, src);
                let constraint_keyword = cursor.node().text();

                cursor.goto_next_sibling();
                // cursor -> name
                pg_ensure_kind!(cursor, SyntaxKind::name, src);

                let constraint_name = cursor.node().text();

                ConflictTarget::OnConstraint(OnConstraint::new(
                    (
                        convert_keyword_case(on_keyword),
                        convert_keyword_case(constraint_keyword),
                    ),
                    constraint_name.to_string(),
                ))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_opt_conf_expr: unexpected node \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_conf_expr, src);

        Ok(conflict_target)
    }

    /// 呼出し時、 cursor は '(' を指している
    /// 呼出し後、 cursor は ')' を指している
    fn handle_conflict_target_column_list_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTargetColumnList, UroboroSQLFmtError> {
        // '(' index_params ')'
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        pg_ensure_kind!(cursor, SyntaxKind::index_params, src);

        let elements = self.visit_index_params(cursor, src)?;

        cursor.goto_next_sibling();
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        // 閉じ括弧の位置まで Location を更新
        loc.append(Location::from(cursor.node().range()));

        Ok(ConflictTargetColumnList::new(elements, loc))
    }

    /// index_params をフォーマットする
    /// index_params はインデックス定義において汎用的に利用される構文だが、
    /// 現状は ON CONFLICT における カラムリスト指定にしか使用していないため Vec<ConflictTargetElement> を返す
    fn visit_index_params(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<ConflictTargetElement>, UroboroSQLFmtError> {
        // index_params:
        // - index_elem (',' index_elem)*

        let mut elements = vec![];

        cursor.goto_first_child();

        let first = self.visit_index_elem(cursor, src)?;
        elements.push(first);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::index_elem => {
                    let index_elem = self.visit_index_elem(cursor, src)?;
                    elements.push(index_elem);
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_index_params: unexpected node \n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::index_params, src);

        Ok(elements)
    }

    /// index_elem はインデックス定義において汎用的に利用される構文だが、
    /// 現状は ON CONFLICT における カラムリスト指定にしか使用していないため ConflictTargetElement を返す
    fn visit_index_elem(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictTargetElement, UroboroSQLFmtError> {
        // index_elem:
        // - ColId index_elem_options
        // - func_expr_windowless index_elem_options
        // - '(' a_expr ')' index_elem_options

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::ColId => {
                let column = convert_identifier_case(cursor.node().text());
                let mut element = ConflictTargetElement::new(column);

                cursor.goto_next_sibling();

                if cursor.node().kind() == SyntaxKind::index_elem_options {
                    let (collate, qualified_name) =
                        self.handle_index_elem_options_nodes(cursor, src)?;

                    if let Some(collate) = collate {
                        element.set_collate(collate);
                    }
                    if let Some(op_class) = qualified_name {
                        element.set_op_class(op_class);
                    }
                }

                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::index_elem, src);

                Ok(element)
            }
            SyntaxKind::func_expr_windowless => {
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_index_elem: func_expr_windowless is not implemented \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::LParen => {
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_index_elem: '(' is not implemented \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_index_elem: unexpected node \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        }
    }

    fn handle_index_elem_options_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<(Option<Collate>, Option<String>), UroboroSQLFmtError> {
        // index_elem_options:
        // - opt_collate? opt_qualified_name? opt_asc_desc? opt_nulls_order?
        // - opt_collate? any_name reloptions opt_asc_desc? opt_nulls_order?

        // 現状は opt_collate と opt_qualified_name のみをサポート

        cursor.goto_first_child();

        // cursor -> opt_collate?
        let collate = if cursor.node().kind() == SyntaxKind::opt_collate {
            let collate = self.visit_opt_collate(cursor, src)?;
            cursor.goto_next_sibling();
            Some(collate)
        } else {
            None
        };

        // cursor -> opt_qualified_name?
        let qualified_name = if cursor.node().kind() == SyntaxKind::opt_qualified_name {
            let op_class = convert_keyword_case(cursor.node().text());
            cursor.goto_next_sibling();
            Some(op_class)
        } else {
            None
        };

        // opt_collate と opt_qualified_name 以外は Unimplemented Error
        match cursor.node().kind() {
            SyntaxKind::any_name
            | SyntaxKind::reloptions
            | SyntaxKind::opt_asc_desc
            | SyntaxKind::opt_nulls_order => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "handle_index_elem_options_nodes: not implemented \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {}
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::index_elem_options, src);

        Ok((collate, qualified_name))
    }

    fn visit_opt_collate(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Collate, UroboroSQLFmtError> {
        // opt_collate:
        // - 'COLLATE' any_name

        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::COLLATE, src);
        let collate_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();
        pg_ensure_kind!(cursor, SyntaxKind::any_name, src);
        let collation = convert_identifier_case(cursor.node().text());

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_collate, src);

        Ok(Collate::new(collate_keyword, collation))
    }

    /// Conflict 句における DO 以降のノードを処理する
    /// 呼出し時、 cursor は DO を指していること
    /// 呼出し後、 cursor は NOTHING または where_clause を指している
    fn handle_conflict_action_nodes(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ConflictAction, UroboroSQLFmtError> {
        // DO UPDATE SET set_clause_list where_clause
        // DO NOTHING

        pg_ensure_kind!(cursor, SyntaxKind::DO, src);
        let do_keyword = cursor.node().text();

        cursor.goto_next_sibling();

        let conflict_action = match cursor.node().kind() {
            SyntaxKind::UPDATE => {
                let update_keyword = cursor.node().text();

                let do_update_keyword = (
                    convert_keyword_case(do_keyword),
                    convert_keyword_case(update_keyword),
                );

                cursor.goto_next_sibling();
                pg_ensure_kind!(cursor, SyntaxKind::SET, src);
                let set_clause = self.handle_set_clause_nodes(cursor, src)?;

                cursor.goto_next_sibling();

                // cursor -> where_clause?
                let where_clause = if cursor.node().kind() == SyntaxKind::where_clause {
                    Some(self.pg_visit_where_clause(cursor, src)?)
                } else {
                    None
                };

                ConflictAction::DoUpdate(DoUpdate::new(do_update_keyword, set_clause, where_clause))
            }
            SyntaxKind::NOTHING => {
                let nothing_keyword = cursor.node().text();

                let do_nothing_keyword = (
                    convert_keyword_case(do_keyword),
                    convert_keyword_case(nothing_keyword),
                );

                ConflictAction::DoNothing(DoNothing::new(do_nothing_keyword))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "handle_conflict_action_nodes: unexpected node \n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        pg_ensure_kind!(cursor, SyntaxKind::NOTHING | SyntaxKind::where_clause, src);

        Ok(conflict_action)
    }
}
