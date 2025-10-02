use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{
        joined_table::{JoinedTable, Qualifier},
        *,
    },
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
    CONFIG,
};

impl Visitor {
    pub(crate) fn visit_joined_table(
        &mut self,
        cursor: &mut TreeCursor,
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
                ensure_kind!(cursor, SyntaxKind::LParen, src);

                cursor.goto_next_sibling();
                let mut start_comments = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    start_comments.push(comment);
                    cursor.goto_next_sibling();
                }

                let mut joined_table = self.visit_joined_table(cursor, src)?;

                cursor.goto_next_sibling();

                let mut end_comments = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    if !comment.is_block_comment()
                        && comment.loc().is_same_line(&joined_table.loc())
                    {
                        joined_table.add_comment_to_child(comment)?;
                    } else {
                        end_comments.push(comment);
                    }
                    cursor.goto_next_sibling();
                }

                ensure_kind!(cursor, SyntaxKind::RParen, src);

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
                    let comment = Comment::new(cursor.node());
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
                ensure_kind!(cursor, SyntaxKind::JOIN, src);
                keywords.push(convert_keyword_case(cursor.node().text()));

                cursor.goto_next_sibling();
                // cursor -> comment?
                let mut comments_after_join_keyword = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    comments_after_join_keyword.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> table_ref
                let mut right = self.visit_table_ref(cursor, src)?;

                // comments_after_join_keywordの最後の要素がバインドパラメータかどうか調べ、
                // バインドパラメータであればrightにセットする
                if let Some(comment) = comments_after_join_keyword.last() {
                    if comment.is_block_comment() && comment.loc().is_next_to(&right.loc()) {
                        // last()がSome()であるため、pop().unwrap()は必ず成功する
                        right.set_head_comment(comments_after_join_keyword.pop().unwrap());
                    }
                }

                cursor.goto_next_sibling();

                let mut joined_table = JoinedTable::new(
                    loc,
                    left,
                    comments_after_left,
                    keywords.join(" "),
                    comments_after_join_keyword,
                    right,
                );

                // cursor -> comments?
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    joined_table.add_comment_to_child(comment)?;

                    cursor.goto_next_sibling();
                }

                // cursor -> join_qual
                if cursor.node().kind() == SyntaxKind::join_qual {
                    let join_qualifier = self.visit_join_qual(cursor, src)?;

                    joined_table.set_qualifier(join_qualifier);
                }

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::joined_table, src);

                Ok(Expr::from(joined_table))
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_joined_table(): unexpected node kind\n{}",
                error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    fn visit_join_qual(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Qualifier, UroboroSQLFmtError> {
        // join_qual
        // - ON a_expr
        // - USING '(' name_list ')' opt_alias_clause_for_join_using

        cursor.goto_first_child();

        match cursor.node().kind() {
            SyntaxKind::ON => {
                // ON a_expr

                ensure_kind!(cursor, SyntaxKind::ON, src);
                let on_keyword = convert_keyword_case(cursor.node().text());

                cursor.goto_next_sibling();
                // cursor -> comment?
                let mut comments_after_keyword = vec![];
                while cursor.node().is_comment() {
                    let comment = Comment::new(cursor.node());
                    comments_after_keyword.push(comment);
                    cursor.goto_next_sibling();
                }

                // cursor -> a_expr
                let expr = self.visit_a_expr_or_b_expr(cursor, src)?;

                let qualifier = Qualifier::new(on_keyword, comments_after_keyword, expr.into());

                cursor.goto_parent();
                ensure_kind!(cursor, SyntaxKind::join_qual, src);

                Ok(qualifier)
            }
            SyntaxKind::USING => {
                // USING '(' name_list ')' opt_alias_clause_for_join_using
                Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_join_qual(): USING node appeared. USING is not implemented yet.\n{}",
                    error_annotation_from_cursor(cursor, src)
                )))
            }
            _ => Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                "visit_join_qual(): unexpected node kind\n{}",
                error_annotation_from_cursor(cursor, src)
            ))),
        }
    }

    fn join_type(
        &mut self,
        cursor: &mut TreeCursor,
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
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::join_type, src);

        Ok(keywords)
    }
}
