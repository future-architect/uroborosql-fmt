use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, Body, Comment, Expr, InsertBody, Location, PrimaryExpr, SeparatedLines,
        Statement,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, COMMA},
    util::{convert_identifier_case, convert_keyword_case},
    NewVisitor as Visitor,
};

use super::SelectStmtOutput;

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
            let with_clause = self.visit_opt_with_clause(cursor, src)?;
            statement.add_clause(with_clause);

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
        let mut insert_body = InsertBody::new(loc, insert_target);

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
                match query {
                    SelectStmtOutput::Statement(stmt) => {
                        insert_body.set_query(stmt);
                    }
                    SelectStmtOutput::Expr(expr) => {
                        insert_body.set_paren_query(expr);
                    }
                    SelectStmtOutput::Values(kw, body) => {
                        insert_body.set_values_clause(&kw, body);
                    }
                }
            }

            cursor.goto_next_sibling();
        }

        // cursor -> opt_on_conflict?
        if cursor.node().kind() == SyntaxKind::opt_on_conflict {
            // let on_conflict = self.visit_on_conflict(cursor, src)?;
            // insert_body.set_on_conflict(on_conflict);

            // cursor.goto_next_sibling();

            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_insert_stmt(): opt_on_conflict is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        into_keyword_clause.set_body(Body::Insert(Box::new(insert_body)));
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
    ) -> Result<(Option<SeparatedLines>, Option<SelectStmtOutput>), UroboroSQLFmtError> {
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

                let mut comments_before_query = Vec::new();
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    comments_before_query.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> SelectStmt | OVERRIDING
                let query = match cursor.node().kind() {
                    SyntaxKind::SelectStmt => {
                        let mut stmt_output = self.visit_select_stmt(cursor, src)?;

                        // コメントを追加（Statement の場合のみ考慮）
                        if let SelectStmtOutput::Statement(ref mut stmt) = stmt_output {
                            for comment in comments_before_query {
                                stmt.add_comment(comment);
                            }
                        } else if !comments_before_query.is_empty() {
                            return Err(UroboroSQLFmtError::Unimplemented(format!(
                                "visit_insert_rest: comments are not supported in this position. \n{}",
                                pg_error_annotation_from_cursor(cursor, src)
                            )));
                        }

                        stmt_output
                    }
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
}
