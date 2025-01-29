use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::{select::SelectBody, *},
    error::UroboroSQLFmtError,
    new_visitor::{
        create_clause, ensure_kind,
        expr::{ComplementConfig, ComplementKind},
        pg_create_clause, pg_ensure_kind, Visitor,
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
        let mut clause = pg_create_clause(cursor, src, "SELECT")?;
        cursor.goto_next_sibling();

        // // SQL_IDとコメントを消費
        // self.consume_or_complement_sql_id(cursor, src, &mut clause);
        // self.consume_comment_in_clause(cursor, src, &mut clause)?;

        let select_body = SelectBody::new();

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

        // // cursor -> select_caluse_body
        // if cursor.node().kind() == "select_clause_body" {
        //     let select_clause_body = self.visit_select_clause_body(cursor, src)?;
        //     select_body.set_select_clause_body(select_clause_body)
        // }

        clause.set_body(Body::Select(Box::new(select_body)));

        // cursor.goto_parent(); // SelectStmt goto parent しちゃだめ
        // pg_ensure_kind(cursor, SyntaxKind::SelectStmt, src)?;

        cursor.goto_next_sibling(); // select の次

        Ok(clause)
    }

    /// SELECT句の本体をSeparatedLinesで返す
    /// 呼び出し後、cursorはselect_clause_bodyを指している
    fn visit_select_clause_body(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // select_clause_bodyは必ず_aliasable_expressionを子供に持つ
        cursor.goto_first_child();

        // cursor -> _aliasable_expression
        // commaSep1(_aliasable_expression)
        // カラム名ルール(ASがなければASを補完)でエイリアス補完、AS補完を行う
        let complement_config = ComplementConfig::new(ComplementKind::ColumnName, true, true);
        let body = self.visit_comma_sep_alias(cursor, src, Some(&complement_config))?;

        // cursorをselect_clause_bodyに
        cursor.goto_parent();
        ensure_kind(cursor, "select_clause_body", src)?;

        Ok(body)
    }
}
