use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{
        from_list::{FromList, TableRef},
        joined_table::{JoinedTable, Qualifier},
        *,
    },
    error::UroboroSQLFmtError,
    new_visitor::{
        pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor, COMMA,
    },
    util::convert_keyword_case,
    CONFIG,
};

impl Visitor {
    pub(crate) fn visit_from_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clause = "FROM" from_list

        // cursor -> "FROM"
        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::FROM, src);

        let mut clause = pg_create_clause!(cursor, SyntaxKind::FROM);
        cursor.goto_next_sibling();

        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> Comma?
        let extra_leading_comma = if cursor.node().kind() == SyntaxKind::Comma {
            cursor.goto_next_sibling();
            Some(COMMA.to_string())
        } else {
            None
        };

        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> from_list
        pg_ensure_kind!(cursor, SyntaxKind::from_list, src);

        let from_list = self.visit_from_list(cursor, src, extra_leading_comma)?;

        clause.set_body(from_list);

        // cursor -> from_clause
        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::from_clause, src);

        Ok(clause)
    }

    /// 呼出し後、cursor は from_list を指している
    /// 直前にカンマがある場合は extra_leading_comma として渡す
    pub(crate) fn visit_from_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
        extra_leading_comma: Option<String>,
    ) -> Result<Body, UroboroSQLFmtError> {
        // from_list -> table_ref ("," table_ref)*

        // from_listは必ず table_ref を子供に持つ
        // cursor -> table_ref
        cursor.goto_first_child();
        pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);

        let mut from_body = FromList::new();
        from_body.set_extra_leading_comma(extra_leading_comma);

        let table_ref = self.visit_table_ref(cursor, src)?;
        from_body.add_table_ref(table_ref);

        while cursor.goto_next_sibling() {
            // cursor -> "," または table_ref
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::table_ref => {
                    let table_ref = self.visit_table_ref(cursor, src)?;
                    from_body.add_table_ref(table_ref);
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    from_body.add_comment_to_child(comment)?;
                }
                SyntaxKind::C_COMMENT => {
                    let comment_node = cursor.node();
                    let comment = Comment::pg_new(comment_node);

                    let Some(next_sibling) = cursor.node().next_sibling() else {
                        // 最後の要素の行末にあるコメントは、 from_list の直下に現れず from_list と同階層の要素になる
                        // そのためコメントが最後の子供になることはなく、次のノードを必ず取得できる
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_from_list(): unexpected node kind\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    };

                    // テーブル参照における置換文字列
                    if comment.loc().is_next_to(&next_sibling.range().into()) {
                        cursor.goto_next_sibling();
                        // cursor -> table_ref
                        pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);
                        let mut table_ref = self.visit_table_ref(cursor, src)?;

                        // 置換文字列をセット
                        table_ref.set_head_comment(comment);
                        from_body.add_table_ref(table_ref);
                    } else {
                        from_body.add_comment_to_child(comment)?;
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_from_list(): unexpected node kind: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // cursor -> from_list
        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::from_list, src);

        Ok(Body::FromList(from_body))
    }

    /// 呼出し後、cursor は table_ref を指している
    fn visit_table_ref(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<TableRef, UroboroSQLFmtError> {
        // table_ref
        // - relation_expr opt_alias_clause [tablesample_clause]
        // - select_with_parens opt_alias_clause
        // - joined_table
        // - '(' joined_table ')' alias_clause
        // - func_table func_alias_clause
        // - xmltable opt_alias_clause
        // - json_table opt_alias_clause
        // - LATERAL func_table func_alias_clause
        // - LATERAL xmltable opt_alias_clause
        // - LATERAL select_with_parens opt_alias_clause
        // - LATERAL json_table opt_alias_clause

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::relation_expr => {
                // 通常のテーブル参照
                // relation_expr opt_alias_clause [tablesample_clause]

                let table_name = self.visit_relation_expr(cursor, src)?;
                let mut table_ref = table_name.to_aligned();

                cursor.goto_next_sibling();

                // cursor -> comment?
                // エイリアスの直前にコメントが来る場合
                if cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());

                    // 行末以外のコメント（次行以降のコメント）は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    if !comment.is_block_comment() && comment.loc().is_same_line(&table_ref.loc()) {
                        table_ref.set_lhs_trailing_comment(comment)?;
                    } else {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_ref(): unexpected comment\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    cursor.goto_next_sibling();
                }

                // cursor -> opt_alias_clause?
                if cursor.node().kind() == SyntaxKind::opt_alias_clause {
                    // opt_alias_clause
                    // - alias_clause
                    cursor.goto_first_child();
                    let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                    // AS補完
                    if let Some(as_keyword) = as_keyword {
                        // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
                        if CONFIG.read().unwrap().remove_table_as_keyword {
                            table_ref.add_rhs(None, col_id);
                        } else {
                            table_ref.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        table_ref.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    pg_ensure_kind!(cursor, SyntaxKind::opt_alias_clause, src);
                    cursor.goto_next_sibling();
                }

                // cursor -> tablesample_clause?
                if cursor.node().kind() == SyntaxKind::tablesample_clause {
                    // TABLESAMPLE
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_table_ref(): tablesample_clause node appeared. Tablesample is not implemented yet.\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }

                cursor.goto_parent();
                // cursor -> table_ref
                pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(table_ref))
            }
            SyntaxKind::select_with_parens => {
                // サブクエリ
                // select_with_parens opt_alias_clause

                let sub_query = self.visit_select_with_parens(cursor, src)?;
                let mut table_ref = sub_query.to_aligned();

                cursor.goto_next_sibling();

                // cursor -> comment?
                // エイリアスの直前にコメントが来る場合
                if cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());

                    // 行末以外のコメント（次行以降のコメント）は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    if !comment.is_block_comment() && comment.loc().is_same_line(&table_ref.loc()) {
                        table_ref.set_lhs_trailing_comment(comment)?;
                    } else {
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_table_ref(): unexpected comment\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    cursor.goto_next_sibling();
                }

                // cursor -> opt_alias_clause?
                if cursor.node().kind() == SyntaxKind::opt_alias_clause {
                    // opt_alias_clause
                    // - alias_clause
                    cursor.goto_first_child();

                    let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                    if let Some(as_keyword) = as_keyword {
                        // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
                        if CONFIG.read().unwrap().remove_table_as_keyword {
                            table_ref.add_rhs(None, col_id);
                        } else {
                            table_ref.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        table_ref.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    pg_ensure_kind!(cursor, SyntaxKind::opt_alias_clause, src);
                }

                cursor.goto_parent();
                // cursor -> table_ref
                pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(table_ref))
            }
            SyntaxKind::joined_table => {
                // テーブル結合
                let joined_table = self.visit_joined_table(cursor, src)?;

                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);

                // joined_tableがExpr::JoinedTableの場合はTableRef::JoinedTableに、
                // それ以外（括弧付きなど）はTableRef::SimpleTableに変換
                match joined_table {
                    Expr::JoinedTable(joined_table_box) => {
                        Ok(TableRef::JoinedTable(joined_table_box))
                    }
                    other => Ok(TableRef::SimpleTable(other.to_aligned())),
                }
            }
            SyntaxKind::LParen => {
                // 括弧付き結合
                // '(' joined_table ')' alias_clause

                cursor.goto_next_sibling();

                let joined_table = self.visit_joined_table(cursor, src)?;
                // ParenExpr を作成
                let parenthesized_joined_table =
                    ParenExpr::new(joined_table, Location::from(cursor.node().range()));

                let mut paren = Expr::ParenExpr(Box::new(parenthesized_joined_table));

                cursor.goto_next_sibling();
                // cursor -> comment?
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    paren.add_comment_to_child(comment)?;
                    cursor.goto_next_sibling();
                }

                pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

                let mut aligned = paren.to_aligned();

                cursor.goto_next_sibling();
                pg_ensure_kind!(cursor, SyntaxKind::alias_clause, src);
                let (as_keyword, col_id) = self.visit_alias_clause(cursor, src)?;

                // as の補完はしない。as が存在し、 remove_table_as_keyword が有効ならば AS を除去
                if let Some(as_keyword) = as_keyword {
                    if CONFIG.read().unwrap().remove_table_as_keyword {
                        aligned.add_rhs(None, col_id);
                    } else {
                        aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                    }
                } else {
                    aligned.add_rhs(None, col_id);
                }

                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::table_ref, src);

                Ok(TableRef::SimpleTable(aligned))
            }
            SyntaxKind::func_table => {
                // テーブル関数呼び出し
                // func_table func_alias_clause
                // - generate_series(1, 10) as g(val)
                // - unnest(array[1, 2, 3]) as nums(n)
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): func_table node appeared. Table function calls are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::LATERAL_P => {
                // LATERAL系
                // LATERAL func_table func_alias_clause
                // LATERAL xmltable opt_alias_clause
                // LATERAL select_with_parens opt_alias_clause
                // LATERAL json_table opt_alias_clause
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): LATERAL_P node appeared. LATERAL expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            SyntaxKind::xmltable => {
                // XMLテーブル
                // xmltable opt_alias_clause
                // - XMLTABLE('/root/row' PASSING data COLUMNS id int PATH '@id')
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): xmltable node appeared. XML tables are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => {
                // TODO: json_table
                Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_table_ref(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        }
    }

    /// alias_clause を visit し、 as キーワード (Option) と Expr を返す
    /// 呼出し後、cursor は alias_clause を指している
    fn visit_alias_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<(Option<String>, Expr), UroboroSQLFmtError> {
        // alias_clause
        // - [AS] ColId ['(' name_list ')']

        // cursor -> alias_clause
        pg_ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        cursor.goto_first_child();
        // cursor -> AS?
        let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = cursor.node().text().to_string();
            cursor.goto_next_sibling();

            Some(as_keyword)
        } else {
            None
        };

        // cursor -> ColId
        let col_id = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
        cursor.goto_next_sibling();

        // cursor -> '('?
        if cursor.node().kind() == SyntaxKind::LParen {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_alias_clause(): {} node appeared. Name lists are not implemented yet.\n{}",
                cursor.node().kind(),
                pg_error_annotation_from_cursor(cursor, src)
            )));
        }

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::alias_clause, src);

        Ok((as_keyword, col_id.into()))
    }

    fn visit_joined_table(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // joined_table
        // - '(' joined_table ')'
        // - table_ref NATURAL join_type? JOIN table_ref
        // - table_ref CROSS JOIN table_ref
        // - table_ref join_type JOIN table_ref join_qual
        // - table_ref JOIN table_ref join_qual
        //
        // join_qual
        // - ON a_expr
        // - USING '(' name_list ')' opt_alias_clause_for_join_using

        let loc = Location::from(cursor.node().range());

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::LParen => {
                // '(' joined_table ')'
                pg_ensure_kind!(cursor, SyntaxKind::LParen, src);

                cursor.goto_next_sibling();
                let mut start_comments = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    start_comments.push(comment);
                    cursor.goto_next_sibling();
                }

                let mut joined_table = self.visit_joined_table(cursor, src)?;

                cursor.goto_next_sibling();

                let mut end_comments = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    if !comment.is_block_comment()
                        && comment.loc().is_same_line(&joined_table.loc())
                    {
                        joined_table.add_comment_to_child(comment)?;
                    } else {
                        end_comments.push(comment);
                    }
                    cursor.goto_next_sibling();
                }

                pg_ensure_kind!(cursor, SyntaxKind::RParen, src);

                cursor.goto_parent();

                let mut paren_expr = ParenExpr::new(joined_table, loc);

                // コメントを追加
                for comment in start_comments {
                    paren_expr.add_start_comment(comment);
                }
                for comment in end_comments {
                    paren_expr.add_end_comment(comment);
                }

                Ok(Expr::ParenExpr(Box::new(paren_expr)))
            }
            SyntaxKind::table_ref => {
                // table_ref NATURAL join_type? JOIN table_ref
                // table_ref CROSS JOIN table_ref
                // table_ref join_type JOIN table_ref join_qual
                // table_ref JOIN table_ref join_qual

                let mut left = self.visit_table_ref(cursor, src)?;

                cursor.goto_next_sibling();

                // cursor -> comment?
                let mut comments_after_left = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    // 行末コメントであれば左辺に追加
                    if !comment.is_block_comment() && comment.loc().is_same_line(&left.loc()) {
                        left.set_trailing_comment(comment)?;
                    } else {
                        comments_after_left.push(comment);
                    }
                    cursor.goto_next_sibling();
                }

                let mut keywords = vec![];

                // cursor -> (CROSS | NATURAL)?
                if matches!(
                    cursor.node().kind(),
                    SyntaxKind::CROSS | SyntaxKind::NATURAL
                ) {
                    keywords.push(convert_keyword_case(cursor.node().text()));
                    cursor.goto_next_sibling();
                }

                // cursor -> join_type?
                if cursor.node().kind() == SyntaxKind::join_type {
                    // join_type
                    // - (FULL | LEFT | RIGHT) OUTER?
                    // - INNER

                    let join_type_texts = self.join_type(cursor, src)?;

                    keywords.extend(join_type_texts);
                    cursor.goto_next_sibling();
                }

                // cursor -> JOIN
                pg_ensure_kind!(cursor, SyntaxKind::JOIN, src);
                keywords.push(convert_keyword_case(cursor.node().text()));

                cursor.goto_next_sibling();
                // cursor -> comment?
                let mut comments_after_join_keyword = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    comments_after_join_keyword.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> table_ref
                let mut right = self.visit_table_ref(cursor, src)?;

                cursor.goto_next_sibling();

                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    if !comment.is_block_comment() && comment.loc().is_same_line(&right.loc()) {
                        right.set_trailing_comment(comment)?;
                    } else {
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_joined_table(): unexpected comment\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                    cursor.goto_next_sibling();
                }

                let mut joined_table = JoinedTable::new(
                    loc,
                    left,
                    comments_after_left,
                    keywords.join(" "),
                    comments_after_join_keyword,
                    right,
                );

                // cursor -> join_qual
                if cursor.node().kind() == SyntaxKind::join_qual {
                    let join_qualifier = self.visit_join_qual(cursor, src)?;

                    joined_table.set_qualifier(join_qualifier);
                }

                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::joined_table, src);

                Ok(Expr::from(joined_table))
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_joined_table(): unexpected node kind\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    fn visit_join_qual(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Qualifier, UroboroSQLFmtError> {
        // join_qual
        // - ON a_expr
        // - USING '(' name_list ')' opt_alias_clause_for_join_using

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::ON => {
                // ON a_expr

                pg_ensure_kind!(cursor, SyntaxKind::ON, src);
                let on_keyword = convert_keyword_case(cursor.node().text());

                cursor.goto_next_sibling();
                // cursor -> comment?
                let mut comments_after_keyword = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());
                    comments_after_keyword.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> a_expr
                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;

                let qualifier = Qualifier::new(on_keyword, comments_after_keyword, expr.into());

                cursor.goto_parent();
                pg_ensure_kind!(cursor, SyntaxKind::join_qual, src);

                Ok(qualifier)
            }
            SyntaxKind::USING => {
                // USING '(' name_list ')' opt_alias_clause_for_join_using
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_join_qual(): USING node appeared. USING is not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_join_qual(): unexpected node kind\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    fn join_type(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Vec<String>, UroboroSQLFmtError> {
        // join_type
        // - (FULL | LEFT | RIGHT) opt_outer?
        // - INNER_P

        cursor.goto_first_child();

        let keywords = match cursor.node().kind() {
            SyntaxKind::FULL | SyntaxKind::LEFT | SyntaxKind::RIGHT => {
                let mut keywords = vec![];

                keywords.push(convert_keyword_case(cursor.node().text()));

                // cursor -> opt_outer?
                if cursor.goto_next_sibling() && cursor.node().kind() == SyntaxKind::opt_outer {
                    keywords.push(convert_keyword_case(cursor.node().text()));
                    cursor.goto_next_sibling();
                } else if CONFIG.read().unwrap().complement_outer_keyword {
                    // OUTER キーワードが省略されていて、補完する設定が有効ならば補完する
                    keywords.push(convert_keyword_case("OUTER"));
                }

                keywords
            }
            SyntaxKind::INNER_P => {
                vec![convert_keyword_case(cursor.node().text())]
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_join_type(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind!(cursor, SyntaxKind::join_type, src);

        Ok(keywords)
    }
}
