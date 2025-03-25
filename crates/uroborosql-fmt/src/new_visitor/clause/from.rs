use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind,
        expr::{ComplementConfig, ComplementKind},
        pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor, COMMA,
    },
    util::{convert_identifier_case, convert_keyword_case},
    CONFIG,
};

impl Visitor {
    /// FROM句をClause構造体で返す
    pub(crate) fn visit_from_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clauseは必ずFROMを子供に持つ
        cursor.goto_first_child();

        // cursor -> FROM
        let mut clause = create_clause(cursor, src, "FROM")?;
        cursor.goto_next_sibling();
        self.consume_comment_in_clause(cursor, src, &mut clause)?;

        // cursor -> aliasable_expression
        // commaSep1(_aliasable_expression)

        // ASがあれば除去する
        // エイリアス補完は現状行わない
        let complement_config = ComplementConfig::new(ComplementKind::TableName, true, false);
        let body = self.visit_comma_sep_alias(cursor, src, Some(&complement_config))?;

        clause.set_body(body);

        // cursorをfrom_clauseに戻す
        cursor.goto_parent();
        ensure_kind(cursor, "from_clause", src)?;

        Ok(clause)
    }

    pub(crate) fn pg_visit_from_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // from_clause = "FROM" from_list

        // cursor -> "FROM"
        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::FROM, src)?;

        let mut clause = pg_create_clause(cursor, SyntaxKind::FROM)?;
        cursor.goto_next_sibling();

        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> from_list
        pg_ensure_kind(cursor, SyntaxKind::from_list, src)?;

        let from_list = self.visit_from_list(cursor, src)?;

        clause.set_body(from_list);

        // cursor -> from_clause
        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::from_clause, src)?;

        Ok(clause)
    }

    /// postgresql-cst-parser の from_list を Body::SeparatedLines に変換する
    /// 呼出し後、cursor は from_list を指している
    pub(crate) fn visit_from_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // from_list -> table_ref ("," table_ref)*

        // from_listは必ず table_ref を子供に持つ
        // cursor -> table_ref
        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::table_ref, src)?;

        let mut sep_lines = SeparatedLines::new();

        let table_ref = self.visit_table_ref(cursor, src)?;
        sep_lines.add_expr(table_ref, None, vec![]);

        while cursor.goto_next_sibling() {
            // cursor -> "," または table_ref
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::table_ref => {
                    let table_ref = self.visit_table_ref(cursor, src)?;
                    sep_lines.add_expr(table_ref, Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
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

                    if comment.loc().is_next_to(&next_sibling.range().into()) {
                        // テーブル参照における置換文字列
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_from_list(): table_ref node with bind parameters appeared. Table references with bind parameters are not implemented yet.\n{}",
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    } else {
                        sep_lines.add_comment_to_child(comment)?;
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
        pg_ensure_kind(cursor, SyntaxKind::from_list, src)?;

        Ok(Body::SepLines(sep_lines))
    }

    /// postgresql-cst-parser の table_ref を Body::SeparatedLines に変換する
    /// 呼出し後、cursor は table_ref を指している
    fn visit_table_ref(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
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
                // テーブル参照
                // relation_expr [opt_alias_clause] [tablesample_clause]

                // AlignedExpr での左辺にあたる式
                let lhs_expr = self.visit_relation_expr(cursor, src)?;

                let mut aligned = AlignedExpr::new(lhs_expr);

                cursor.goto_next_sibling();

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
                            aligned.add_rhs(None, col_id);
                        } else {
                            aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        aligned.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    pg_ensure_kind(cursor, SyntaxKind::opt_alias_clause, src)?;
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
                pg_ensure_kind(cursor, SyntaxKind::table_ref, src)?;

                Ok(aligned)
            }
            SyntaxKind::select_with_parens => {
                // サブクエリ
                // select_with_parens opt_alias_clause

                let mut aligned = AlignedExpr::new(self.visit_select_with_parens(cursor, src)?);

                cursor.goto_next_sibling();

                // cursor -> comment?
                // エイリアスの直前にコメントが来る場合
                if cursor.node().is_comment() {
                    let comment = Comment::pg_new(cursor.node());

                    // 行末以外のコメント（次行以降のコメント）は未定義
                    // 通常、エイリアスの直前に複数コメントが来るような書き方はしないため未対応
                    if !comment.is_block_comment() && comment.loc().is_same_line(&aligned.loc()) {
                        aligned.set_lhs_trailing_comment(comment)?;
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
                            aligned.add_rhs(None, col_id);
                        } else {
                            aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), col_id);
                        }
                    } else {
                        // ASが無い場合は補完しない
                        aligned.add_rhs(None, col_id);
                    }

                    cursor.goto_parent();
                    pg_ensure_kind(cursor, SyntaxKind::opt_alias_clause, src)?;
                }

                cursor.goto_parent();
                // cursor -> table_ref
                pg_ensure_kind(cursor, SyntaxKind::table_ref, src)?;

                Ok(aligned)
            }
            SyntaxKind::joined_table => {
                // テーブル結合
                // joined_table
                // - users INNER JOIN orders ON users.id = orders.user_id
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): joined_table node appeared. Table joins are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::LParen => {
                // 括弧付き結合
                // '(' joined_table ')' alias_clause
                // - (users JOIN orders ON users.id = orders.user_id) AS uo
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): parenthesized join node appeared. Parenthesized joins are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::func_table => {
                // テーブル関数呼び出し
                // func_table func_alias_clause
                // - generate_series(1, 10) as g(val)
                // - unnest(array[1, 2, 3]) as nums(n)
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): func_table node appeared. Table function calls are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::LATERAL_P => {
                // LATERAL系
                // LATERAL func_table func_alias_clause
                // LATERAL xmltable opt_alias_clause
                // LATERAL select_with_parens opt_alias_clause
                // LATERAL json_table opt_alias_clause
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): LATERAL_P node appeared. LATERAL expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            SyntaxKind::xmltable => {
                // XMLテーブル
                // xmltable opt_alias_clause
                // - XMLTABLE('/root/row' PASSING data COLUMNS id int PATH '@id')
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_table_ref(): xmltable node appeared. XML tables are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                // TODO: json_table
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_table_ref(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        }
    }

    fn visit_relation_expr(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // relation_expr
        // - qualified_name
        // - extended_relation_expr

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::qualified_name => self.visit_qualified_name(cursor, src)?,
            SyntaxKind::extended_relation_expr => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_relation_expr(): extended_relation_expr node appeared. Extended relation expressions are not implemented yet.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_relation_expr(): unexpected node kind\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::relation_expr, src)?;

        Ok(expr)
    }

    fn visit_qualified_name(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // qualified_name
        // - ColId
        // - ColId indirection

        cursor.goto_first_child();
        pg_ensure_kind(cursor, SyntaxKind::ColId, src)?;

        let mut qualified_name_text = cursor.node().text().to_string();

        if cursor.goto_next_sibling() {
            // indirection が存在する場合
            pg_ensure_kind(cursor, SyntaxKind::indirection, src)?;

            let indirection_text = cursor.node().text().to_string();

            if indirection_text.contains('[') {
                // この場所での subscript （[1] など）は構文定義上可能だが、PostgreSQL側でrejectされる不正な記述
                // - https://github.com/postgres/postgres/blob/master/src/backend/parser/gram.y#L17303-L17304
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_qualified_name(): invalid subscript notation appeared.\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            // 空白を除去してqualified_name_textに追加
            qualified_name_text.push_str(
                &indirection_text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>(),
            );
        }

        cursor.goto_parent();
        // cursor -> qualified_name
        pg_ensure_kind(cursor, SyntaxKind::qualified_name, src)?;

        let primary = PrimaryExpr::new(
            convert_identifier_case(&qualified_name_text),
            cursor.node().range().into(),
        );

        Ok(primary.into())
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
        pg_ensure_kind(cursor, SyntaxKind::alias_clause, src)?;

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
        pg_ensure_kind(cursor, SyntaxKind::alias_clause, src)?;

        Ok((as_keyword, col_id.into()))
    }
}
