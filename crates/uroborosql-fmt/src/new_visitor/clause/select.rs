use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{select::SelectBody, *},
    error::UroboroSQLFmtError,
    new_visitor::{
        pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor, Visitor, COMMA,
    },
};

impl Visitor {
    /// SELECT句
    /// 呼び出し後、cursorはselect_clauseを指している
    pub(crate) fn visit_select_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // SELECT句の定義
        //      select_clause =
        //          SELECT
        //          [ ALL | DISTINCT [ ON ( expression [, ...] ) ] ] ]
        //          [ select_clause_body ]

        pg_ensure_kind(cursor, SyntaxKind::SELECT, src)?;

        // cursor -> SELECT
        let mut clause = pg_create_clause(cursor, SyntaxKind::SELECT)?;
        cursor.goto_next_sibling();

        // // SQL_IDとコメントを消費
        // self.consume_or_complement_sql_id(cursor, src, &mut clause);
        // self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let mut select_body = SelectBody::new();

        // TODO: all, distinct
        // // [ ALL | DISTINCT [ ON ( expression [, ...] ) ] ] ]
        // match cursor.node().kind() {
        //     "ALL" => {
        //         let all_clause = create_clause(cursor, src, "ALL")?;

        //         select_body.set_all_distinct(all_clause);

        //         cursor.goto_next_sibling();
        //     }
        //     "DISTINCT" => {
        //         let mut distinct_clause = create_clause(cursor, src, "DISTINCT")?;

        //         cursor.goto_next_sibling();

        //         // ON ( expression [, ...] )
        //         if cursor.node().kind() == "ON" {
        //             // DISTINCTにONキーワードを追加
        //             distinct_clause.extend_kw(cursor.node(), src);

        //             cursor.goto_next_sibling();

        //             // ( expression [, ...] ) をColumnList構造体に格納
        //             let mut column_list = self.visit_column_list(cursor, src)?;
        //             // 改行によるフォーマットを強制
        //             column_list.set_force_multi_line(true);

        //             // ColumntListをSeparatedLinesに格納してBody
        //             let mut sep_lines = SeparatedLines::new();

        //             sep_lines.add_expr(
        //                 Expr::ColumnList(Box::new(column_list)).to_aligned(),
        //                 None,
        //                 vec![],
        //             );

        //             distinct_clause.set_body(Body::SepLines(sep_lines));
        //         }

        //         select_body.set_all_distinct(distinct_clause);

        //         cursor.goto_next_sibling();
        //     }
        //     _ => {}
        // }

        // cursor -> target_list
        if cursor.node().kind() == SyntaxKind::target_list {
            let target_list = self.visit_target_list(cursor, src)?;
            // select_clause_body 部分に target_list から生成した Body をセット
            select_body.set_select_clause_body(target_list);
        }

        clause.set_body(Body::Select(Box::new(select_body)));

        // cursor.goto_parent(); // SelectStmt goto parent しちゃだめ
        // pg_ensure_kind(cursor, SyntaxKind::SelectStmt, src)?;

        cursor.goto_next_sibling(); // select の次

        Ok(clause)
    }

    /// [pg] postgresql-cst-parser の target_list を Body::SeparatedLines に変換する
    /// tree-sitter の select_clause_body が該当
    /// 呼び出し後、cursorは target_list を指している
    fn visit_target_list(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // target_list -> target_el ("," target_el)*

        // target_listは必ずtarget_elを子供に持つ
        cursor.goto_first_child();

        // cursor -> target_el
        let mut sep_lines = SeparatedLines::new();

        let target_el = self.visit_target_el(cursor, src)?;
        sep_lines.add_expr(target_el, None, vec![]);

        while cursor.goto_next_sibling() {
            // cursor -> "," または target_el
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::target_el => {
                    let target_el = self.visit_target_el(cursor, src)?;
                    sep_lines.add_expr(target_el, Some(COMMA.to_string()), vec![]);
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    sep_lines.add_comment_to_child(comment)?;
                }
                SyntaxKind::C_COMMENT => {
                    // TODO:バインドパラメータ判定を含む実装
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_target_list(): C_COMMENT is not implemented\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_target_list(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        // cursorをtarget_listに
        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::target_list, &src)?;

        Ok(Body::SepLines(sep_lines))
    }

    fn visit_target_el(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<AlignedExpr, UroboroSQLFmtError> {
        //
        // target_el
        // - a_expr AS ColLabel
        // - a_expr BareColLabel
        // - a_expr
        // - Star
        //

        cursor.goto_first_child();

        let expr = match cursor.node().kind() {
            SyntaxKind::a_expr => self.visit_a_expr(cursor, src)?,
            SyntaxKind::Star => {
                // Star は postgresql-cst-parser の語彙で、uroborosql-fmt::cst では AsteriskExpr として扱う
                // Star は postgres の文法上 Expression ではないが、 cst モジュールの Expr に変換する
                let asterisk =
                    AsteriskExpr::new(cursor.node().text(), cursor.node().range().into());

                Expr::Asterisk(Box::new(asterisk))
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_target_el(): excepted node is {}, but actual {}\n{}",
                    SyntaxKind::target_el,
                    cursor.node().kind(),
                    pg_error_annotation_from_cursor(cursor, src)
                )))
            }
        };

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::target_el, src)?;

        Ok(AlignedExpr::new(expr))
    }
}
