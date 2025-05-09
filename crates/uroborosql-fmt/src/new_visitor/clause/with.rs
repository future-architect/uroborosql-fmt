use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        AlignedExpr, Body, Clause, ColumnList, Comment, Cte, Expr, Location, PrimaryExpr,
        PrimaryExprKind, Statement, SubExpr, WithBody,
    },
    error::UroboroSQLFmtError,
    new_visitor::{
        pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor,
        statement::SelectStmtOutput,
    },
    util::{convert_identifier_case, convert_keyword_case},
    NewVisitor as Visitor,
};

// with_clause
// - WITH RECURSIVE? cte_list

// cte_list
// - common_table_expr ( ',' common_table_expr)*
//
// cte_list is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/20

// common_table_expr
// - name opt_name_list? AS opt_materialized? '(' PreparableStmt ')' opt_search_clause? opt_cycle_clause?

// opt_name_list
// - '(' name_list ')'

// name_list
// - name ( ',' name)*
//
// name_list is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/20

// opt_materialized
// - MATERIALIZED
// - NOT MATERIALIZED

// PreparableStmt
// - SelectStmt
// - InsertStmt
// - UpdateStmt
// - DeleteStmt
// - MergeStmt

// opt_search_clause
// - SEARCH search_order SET ColLabel

// opt_cycle_clause
// - CYCLE name_list SET ColLabel opt_equal_to DEFAULT ColLabel opt_using_path

impl Visitor {
    pub(crate) fn visit_opt_with_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // opt_with_clause
        // - with_clause

        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::with_clause, src);

        let with_clause = self.visit_with_clause(cursor, src)?;

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_with_clause, src);

        Ok(with_clause)
    }

    pub(crate) fn visit_with_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // with_clause
        // - (WITH | WITH_LA) RECURSIVE? cte_list

        cursor.goto_first_child();

        let mut with_clause = pg_create_clause!(cursor, SyntaxKind::WITH | SyntaxKind::WITH_LA);

        cursor.goto_next_sibling();

        if cursor.node().kind() == SyntaxKind::RECURSIVE {
            // WITH句のキーワードにRECURSIVEを付与する
            with_clause.pg_extend_kw(cursor.node());
            cursor.goto_next_sibling();
        }

        // SQL_ID とコメントを消費
        self.pg_consume_or_complement_sql_id(cursor, &mut with_clause);
        self.pg_consume_comments_in_clause(cursor, &mut with_clause)?;

        let with_body = self.visit_cte_list(cursor, src)?;
        with_clause.set_body(Body::With(Box::new(with_body)));

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::with_clause, src);

        Ok(with_clause)
    }

    fn visit_cte_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<WithBody, UroboroSQLFmtError> {
        // cte_list
        // - common_table_expr ( ',' common_table_expr)*
        //
        // cte_list is flatten: https://github.com/future-architect/postgresql-cst-parser/pull/20

        cursor.goto_first_child();

        let mut with_body = WithBody::new();

        loop {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::common_table_expr => {
                    let cte = self.visit_common_table_expr(cursor, src)?;
                    with_body.add_cte(cte);
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    with_body.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_cte_list: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::cte_list, src);

        Ok(with_body)
    }

    fn visit_common_table_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Cte, UroboroSQLFmtError> {
        // common_table_expr
        // - name opt_name_list? AS opt_materialized? '(' PreparableStmt ')' opt_search_clause? opt_cycle_clause?

        cursor.goto_first_child();

        // cursor -> name
        let table_name = convert_identifier_case(cursor.node().text());

        cursor.goto_next_sibling();

        // cursor -> opt_name_list?
        let column_name = if cursor.node().kind() == SyntaxKind::opt_name_list {
            let mut column_list = self.visit_opt_name_list(cursor, src)?;

            // WITH句のカラム名指定は複数行で描画する
            column_list.set_force_multi_line(true);

            cursor.goto_next_sibling();
            Some(column_list)
        } else {
            None
        };

        // cursor -> comment? (テーブル名の直後のコメント)
        let name_trailing_comment = if cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            cursor.goto_next_sibling();
            Some(comment)
        } else {
            None
        };

        // cursor -> AS
        pg_ensure_kind!(cursor, SyntaxKind::AS, src);
        let as_keyword = convert_keyword_case(cursor.node().text());

        cursor.goto_next_sibling();

        // cursor -> opt_materialized?
        let materialized_keyword = if cursor.node().kind() == SyntaxKind::opt_materialized {
            // opt_materialized
            // - MATERIALIZED
            // - NOT MATERIALIZED

            let text = cursor.node().text();
            let splitted = text.split_whitespace().collect::<Vec<_>>();

            cursor.goto_next_sibling();
            Some(convert_keyword_case(&splitted.join(" ")))
        } else {
            None
        };

        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
        // `( statement )` の location を作りたいので、開き括弧の location を持っておく
        let mut parenthized_stmt_loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();

        // cursor -> comments?
        let mut comment_buf = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> PreparableStmt
        let mut statement = self.visit_preparable_stmt(cursor, src)?;
        parenthized_stmt_loc.append(Location::from(cursor.node().range()));

        cursor.goto_next_sibling();

        // cursor -> comments? (statement 直後のコメント)
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        parenthized_stmt_loc.append(Location::from(cursor.node().range()));

        // 開き括弧とstatementの間にあるコメントを追加
        for comment in comment_buf {
            statement.add_comment(comment);
        }

        let subexpr = SubExpr::new(statement, parenthized_stmt_loc);

        cursor.goto_next_sibling();

        // cursor -> opt_search_clause?
        if cursor.node().kind() == SyntaxKind::opt_search_clause {
            // opt_search_clause
            // - SEARCH search_order SET ColLabel
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_common_table_expr: opt_search_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> opt_cycle_clause?
        if cursor.node().kind() == SyntaxKind::opt_cycle_clause {
            // opt_cycle_clause
            // - CYCLE name_list SET ColLabel opt_equal_to DEFAULT ColLabel opt_using_path
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_common_table_expr: opt_cycle_clause is not implemented\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::common_table_expr, src);

        let mut cte = Cte::new(
            Location::from(cursor.node().range()),
            table_name,
            as_keyword,
            column_name,
            materialized_keyword,
            subexpr,
        );

        // テーブル名の直後のコメントを追加
        if let Some(comment) = name_trailing_comment {
            cte.set_name_trailing_comment(comment)?;
        }

        Ok(cte)
    }

    fn visit_opt_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // opt_name_list
        // - '(' name_list ')'

        cursor.goto_first_child();

        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

        cursor.goto_next_sibling();
        // cursor -> comment?

        // 開き括弧と式との間にあるコメントを保持
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> name_list
        pg_ensure_kind!(cursor, SyntaxKind::name_list, src);
        let mut exprs = self.visit_name_list(cursor, src)?;

        cursor.goto_next_sibling();
        // cursor -> comment?

        if cursor.node().is_comment() {
            // 行末コメントを想定する
            let comment = Comment::pg_new(cursor.node());

            // exprs は必ず1つ以上要素を持っている
            let last = exprs.last_mut().unwrap();
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment)?;
            } else {
                // 行末コメント以外のコメントは想定していない
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_opt_name_list(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::opt_name_list, src);

        let loc = Location::from(cursor.node().range());
        Ok(ColumnList::new(exprs, loc, start_comments))
    }

    fn visit_name_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // name_list
        // - name ( ',' name)*
        //
        // name: ColId

        cursor.goto_first_child();
        // cursor -> name

        let mut names = vec![];

        pg_ensure_kind!(cursor, SyntaxKind::name, src);
        let first = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
        names.push(Expr::Primary(Box::new(first)).to_aligned());

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::name => {
                    let name = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
                    names.push(Expr::Primary(Box::new(name)).to_aligned());
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // names は必ず1つ以上要素を持っている
                    let last = names.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_name_list(): Unexpected comment\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_name_list: unexpected node kind: {}",
                        cursor.node().kind()
                    )));
                }
            }
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::name_list, src);

        Ok(names)
    }

    fn visit_preparable_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // PreparableStmt
        // - SelectStmt
        // - InsertStmt
        // - UpdateStmt
        // - DeleteStmt
        // - MergeStmt

        cursor.goto_first_child();

        let statement_or_expr = match cursor.node().kind() {
            SyntaxKind::SelectStmt => self.visit_select_stmt(cursor, src)?,
            unimplemented_stmt @ (SyntaxKind::InsertStmt
            | SyntaxKind::UpdateStmt
            | SyntaxKind::DeleteStmt
            | SyntaxKind::MergeStmt) => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_preparable_stmt: {} is not implemented\n{}",
                    unimplemented_stmt,
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_preparable_stmt: unexpected node kind: {}",
                    cursor.node().kind()
                )));
            }
        };

        // 現状は Statement を返すパターンを考慮する
        match statement_or_expr {
            SelectStmtOutput::Statement(statement) => {
                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::PreparableStmt, src);

                Ok(statement)
            }
            _ => Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_preparable_stmt: VALUES clauses or expressions are not supported as PreparableStmt.\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            ))),
        }
    }
}
