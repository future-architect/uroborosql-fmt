use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, Body, ColumnList, Comment, Expr, Location, PrimaryExpr, SeparatedLines,
        Statement,
    },
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, COMMA},
    util::convert_keyword_case,
    NewVisitor as Visitor,
};

// UpdateStmt:
// - opt_with_clause? UPDATE relation_expr_opt_alias SET set_clause_list from_clause? where_or_current_clause? returning_clause?
//
// opt_with_clause:
// - with_clause

impl Visitor {
    pub(crate) fn visit_update_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        let mut statement = Statement::new();

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

        // cursor -> UPDATE
        pg_ensure_kind!(cursor, SyntaxKind::UPDATE, src);
        let mut update_clause = pg_create_clause!(cursor, SyntaxKind::UPDATE);

        cursor.goto_next_sibling();
        self.pg_consume_or_complement_sql_id(cursor, &mut update_clause);
        self.pg_consume_comments_in_clause(cursor, &mut update_clause)?;

        // cursor -> relation_expr_opt_alias
        let body = self.visit_relation_expr_opt_alias(cursor, src)?;
        update_clause.set_body(body);
        statement.add_clause(update_clause);

        cursor.goto_next_sibling();

        // cursor -> comments?
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> SET
        pg_ensure_kind!(cursor, SyntaxKind::SET, src);
        let mut set_clause = pg_create_clause!(cursor, SyntaxKind::SET);
        cursor.goto_next_sibling();

        // キーワード直後のコメントを処理
        self.pg_consume_comments_in_clause(cursor, &mut set_clause)?;

        // cursor -> set_clause_list
        pg_ensure_kind!(cursor, SyntaxKind::set_clause_list, src);
        let set_clause_list = self.visit_set_clause_list(cursor, src)?;

        set_clause.set_body(set_clause_list);
        statement.add_clause(set_clause);

        // from_clause, where_or_current_clause, returning_clause を持つ可能性がある
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::from_clause => {
                    let clause = self.visit_from_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::where_or_current_clause => {
                    let clause = self.visit_where_or_current_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::returning_clause => {
                    let clause = self.visit_returning_clause(cursor, src)?;
                    statement.add_clause(clause);
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_update_stmt(): unexpected syntax\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> UpdateStmt
        pg_ensure_kind!(cursor, SyntaxKind::UpdateStmt, src);

        Ok(statement)
    }

    fn visit_set_clause_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // set_clause_list:
        // - set_clause (',' set_clause)*
        // flattened: https://github.com/future-architect/postgresql-cst-parser/pull/21

        cursor.goto_first_child();
        let mut sep_lines = SeparatedLines::new();

        pg_ensure_kind!(cursor, SyntaxKind::set_clause, src);
        let set_clause = self.visit_set_clause(cursor, src)?;
        sep_lines.add_expr(set_clause, None, vec![]);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::set_clause => {
                    let set_clause = self.visit_set_clause(cursor, src)?;
                    sep_lines.add_expr(set_clause, Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::Comma => {
                    continue;
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_set_clause_list(): unexpected syntax\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> set_clause_list
        pg_ensure_kind!(cursor, SyntaxKind::set_clause_list, src);

        Ok(Body::SepLines(sep_lines))
    }

    fn visit_set_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        // set_clause:
        // - set_target '=' a_expr
        // - '(' set_target_list ')' '=' a_expr

        cursor.goto_first_child();

        // lhs: set_target | '(' set_target_list ')'
        let lhs = match cursor.node().kind() {
            SyntaxKind::set_target => self.visit_set_target(cursor, src)?,
            SyntaxKind::LParen => {
                let column_list = self.handle_parenthesized_set_target_list(cursor, src)?;

                pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
                Expr::ColumnList(Box::new(column_list))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_set_clause(): unexpected syntax\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        let mut aligned = AlignedExpr::new(lhs);

        cursor.goto_next_sibling();
        pg_ensure_kind!(cursor, SyntaxKind::Equals, src);

        cursor.goto_next_sibling();
        let rhs = self.visit_a_expr_or_b_expr(cursor, src)?;

        aligned.add_rhs(Some("=".to_string()), rhs);

        cursor.goto_parent();
        // cursor -> set_clause
        pg_ensure_kind!(cursor, SyntaxKind::set_clause, src);

        Ok(aligned)
    }

    fn visit_set_target(
        &self,
        cursor: &mut TreeCursor,
        _src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // set_target:
        // - ColId opt_indirection

        let location = Location::from(cursor.node().range());

        // 子ノードを個別に走査せず、 set_target にあたるテキスト全体を一括で取得する
        //
        // `target  [  0 ]` をパースした場合:
        // - set_target       : `target  [  0 ]` <- このノード全体のテキストを直接取得
        //   - ColId          : `target`         <- 個別に処理しない
        //   - opt_indirection: `[  0 ]`         <- 個別に処理しない
        let text = cursor.node().text();

        // 単純に空白を削除してフォーマット処理とする
        // 例: `target  [  0 ]` → `target[0]`
        let whitespace_removed_text = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        let expr = PrimaryExpr::new(convert_keyword_case(&whitespace_removed_text), location);

        Ok(Expr::Primary(Box::new(expr)))
    }

    // '(' set_target_list ')' というノードの並びを処理し、 ColumnList を返す
    // 呼出し時、 cursor は '(' を指している
    // 呼出し後、 cursor は ')' を指している
    fn handle_parenthesized_set_target_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // '(' set_target_list ')'

        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        // 開き括弧の位置を保持
        let mut column_list_location = Location::from(cursor.node().range());

        cursor.goto_next_sibling();

        // 開き括弧と最初の式との間にあるコメントを保持
        // 最後の要素はバインドパラメータの可能性があるので、最初の式を処理した後で付け替える
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            start_comments.push(Comment::pg_new(cursor.node()));
            cursor.goto_next_sibling();
        }

        // cursor -> set_target_list
        let mut exprs = self.visit_set_target_list(cursor, src)?;

        // start_comments の最後の要素が exprs の最初の要素のバインドパラメータであれば付与
        if let Some(last_comment) = start_comments.last() {
            if let Some(first_expr) = exprs.first_mut() {
                if last_comment.is_block_comment()
                    && last_comment.loc().is_next_to(&first_expr.loc())
                {
                    // バインドパラメータとして式に付与
                    first_expr.set_head_comment(last_comment.clone());

                    // start_comments からは最後の要素を削除
                    start_comments.pop();
                }
            }
        }

        cursor.goto_next_sibling();
        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        // location を閉じ括弧の位置までに更新
        column_list_location.append(cursor.node().range().into());

        let column_list = ColumnList::new(exprs, column_list_location, start_comments);
        Ok(column_list)
    }

    fn visit_set_target_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // set_target_list:
        // - set_target (',' set_target)*
        // flattened: https://github.com/future-architect/postgresql-cst-parser/pull/21

        cursor.goto_first_child();
        // cursor -> set_target

        let mut exprs = Vec::new();

        // 最初の要素
        pg_ensure_kind!(cursor, SyntaxKind::set_target, src);
        let expr = self.visit_set_target(cursor, src)?;
        exprs.push(expr.to_aligned());

        // 残りの要素
        // cursor -> set_target | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::set_target => {
                    exprs.push(self.visit_set_target(cursor, src)?.to_aligned());
                }
                // バインドパラメータを想定
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // 次の式へ
                    if !cursor.goto_next_sibling() {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_set_target_list(): Unexpected syntax. node: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    // cursor -> set_target
                    pg_ensure_kind!(cursor, SyntaxKind::set_target, src);
                    let mut expr = self.visit_set_target(cursor, src)?;

                    // コメントがバインドパラメータならば式に付与
                    if comment.is_block_comment() && comment.loc().is_next_to(&expr.loc()) {
                        expr.set_head_comment(comment.clone());
                    } else {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_set_target_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    exprs.push(expr.to_aligned());
                }
                // 行末コメント
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_set_target_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_set_target_list(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> set_target_list
        pg_ensure_kind!(cursor, SyntaxKind::set_target_list, src);

        Ok(exprs)
    }
}
