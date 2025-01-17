use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    util::{convert_identifier_case, convert_keyword_case},
    visitor::{create_clause, ensure_kind, error_annotation_from_cursor, Visitor, COMMA, COMMENT},
};

impl Visitor {
    /// WITH句
    pub(crate) fn visit_with_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        cursor.goto_first_child();

        let mut with_clause = create_clause(cursor, src, "WITH")?;

        cursor.goto_next_sibling();

        if cursor.node().kind() == "RECURSIVE" {
            // WITH句のキーワードにRECURSIVEを付与する
            with_clause.extend_kw(cursor.node(), src);
            cursor.goto_next_sibling();
        }

        // SQL_IDとコメントを消費
        self.consume_or_complement_sql_id(cursor, src, &mut with_clause);
        self.consume_comment_in_clause(cursor, src, &mut with_clause)?;

        let mut with_body = WithBody::new();
        loop {
            match cursor.node().kind() {
                COMMA => {}
                "cte" => {
                    let cte = self.visit_cte(cursor, src)?;
                    with_body.add_cte(cte);
                }
                COMMENT => {
                    let comment = Comment::new(cursor.node(), src);

                    with_body.add_comment_to_child(comment)?;
                }
                "ERROR" => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_with_clause: ERROR node appeared \n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    break;
                }
            }

            if !cursor.goto_next_sibling() {
                break;
            }
        }

        with_clause.set_body(Body::With(Box::new(with_body)));

        cursor.goto_parent();
        ensure_kind(cursor, "with_clause", src)?;

        Ok(with_clause)
    }

    /// cte(Common Table Expressions)をフォーマット
    fn visit_cte(&mut self, cursor: &mut TreeCursor, src: &str) -> Result<Cte, UroboroSQLFmtError> {
        let loc = Location::new(cursor.node().range());

        cursor.goto_first_child();
        // cursor -> identifier

        let table_name = convert_identifier_case(cursor.node().utf8_text(src.as_ref()).unwrap());

        cursor.goto_next_sibling();

        let mut column_name = None;

        if cursor.node().kind() == "(" {
            // cursor -> ( column_name [, ...] )
            let mut column_list = self.visit_column_list(cursor, src)?;

            // WITH句のカラム名指定は複数行で描画する
            column_list.set_force_multi_line(true);

            cursor.goto_next_sibling();

            column_name = Some(column_list);
        };

        // テーブル名の直後のコメント
        let mut name_trailing_comment = None;
        if cursor.node().kind() == COMMENT {
            name_trailing_comment = Some(Comment::new(cursor.node(), src));
            cursor.goto_next_sibling();
        }

        // cursor -> "AS"
        ensure_kind(cursor, "AS", src)?;

        let as_keyword = convert_keyword_case(cursor.node().utf8_text(src.as_ref()).unwrap());

        cursor.goto_next_sibling();

        let mut materialized_keyword = None;

        // cursor -> "NOT"?
        if cursor.node().kind() == "NOT" {
            let not = convert_keyword_case(cursor.node().utf8_text(src.as_ref()).unwrap());
            materialized_keyword = Some(not);
            cursor.goto_next_sibling();
        }

        // cursor -> "MATERIALIZED"?
        if cursor.node().kind() == "MATERIALIZED" {
            let materialized = convert_keyword_case(cursor.node().utf8_text(src.as_ref()).unwrap());

            if let Some(materialized_keyword) = &mut materialized_keyword {
                // NOTがある場合にしかこの分岐に入らないのでMATERIALIZEDの前に空白を付与して挿入する
                materialized_keyword.push_str(&format!(" {materialized}"));
            } else {
                // NOTがない場合
                materialized_keyword = Some(materialized);
            }

            cursor.goto_next_sibling();
        }

        // ( statement ) のloc
        let mut stmt_loc = Location::new(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> select_statement | comments

        let mut comment_buf = vec![];
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            comment_buf.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> select_statement | delete_statement | insert_statement | update_statement
        let mut statement = match cursor.node().kind() {
            // TODO: パーサ置き換えのためコメントアウト
            // "select_statement" => self.visit_select_stmt(cursor, src)?,
            "delete_statement" => self.visit_delete_stmt(cursor, src)?,
            "insert_statement" => self.visit_insert_stmt(cursor, src)?,
            "update_statement" => self.visit_update_stmt(cursor, src)?,
            _ => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "visit_cte(): Unimplemented statement\n{}",
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };
        stmt_loc.append(Location::new(cursor.node().range()));

        cursor.goto_next_sibling();

        // statementの最後のコメントを処理する
        while cursor.node().kind() == COMMENT {
            let comment = Comment::new(cursor.node(), src);
            statement.add_comment_to_child(comment)?;
            cursor.goto_next_sibling();
        }

        // cursor -> )
        ensure_kind(cursor, ")", src)?;
        stmt_loc.append(Location::new(cursor.node().range()));

        // 開きかっことstatementの間にあるコメントを追加
        for comment in comment_buf {
            statement.add_comment(comment);
        }

        let subexpr = SubExpr::new(statement, stmt_loc);

        // cursorを戻しておく
        cursor.goto_parent();
        ensure_kind(cursor, "cte", src)?;

        let mut cte = Cte::new(
            loc,
            table_name,
            as_keyword,
            column_name,
            materialized_keyword,
            subexpr,
        );

        if let Some(comment) = name_trailing_comment {
            cte.set_name_trailing_comment(comment)?;
        }

        Ok(cte)
    }
}
