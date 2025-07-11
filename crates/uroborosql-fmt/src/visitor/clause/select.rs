use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::{select::SelectBody, *},
    error::UroboroSQLFmtError,
    visitor::{create_clause, ensure_kind, Visitor, COMMA},
};

impl Visitor {
    /// SELECT句
    /// 呼び出し後、cursor は target_list があれば target_list を、無ければ SELECT キーワードを指している
    pub(crate) fn visit_select_clause(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Clause, UroboroSQLFmtError> {
        // select_clause が無く、すでに select キーワード を指しているため goto_first_child しない
        // context: https://github.com/future-architect/postgresql-cst-parser/pull/2#discussion_r1897026688
        ensure_kind!(cursor, SyntaxKind::SELECT, src);

        // cursor -> SELECT
        let mut clause = create_clause!(cursor, SyntaxKind::SELECT);
        cursor.goto_next_sibling();

        // SQL_IDとコメントを消費
        self.consume_or_complement_sql_id(cursor, &mut clause);
        self.consume_comments_in_clause(cursor, &mut clause)?;

        let mut select_body = SelectBody::new();

        // TODO: opt_distinct_clause の考慮
        // opt_all_clause | distinct_clause
        match cursor.node().kind() {
            SyntaxKind::opt_all_clause => {
                // opt_all_clause
                // - ALL

                cursor.goto_first_child();
                // cursor -> ALL
                ensure_kind!(cursor, SyntaxKind::ALL, src);

                let all_clause = create_clause!(cursor, SyntaxKind::ALL);

                select_body.set_all_distinct(all_clause);

                cursor.goto_parent();
                // cursor -> opt_all_clause
                ensure_kind!(cursor, SyntaxKind::opt_all_clause, src);

                cursor.goto_next_sibling();
            }
            SyntaxKind::distinct_clause => {
                // distinct_clause
                // - DISTINCT
                // - DISTINCT ON '(' expr_list ')'

                cursor.goto_first_child();
                // cursor -> DISTINCT
                ensure_kind!(cursor, SyntaxKind::DISTINCT, src);
                let mut distinct_clause = create_clause!(cursor, SyntaxKind::DISTINCT);

                cursor.goto_next_sibling();

                // cursor -> ON?
                if cursor.node().kind() == SyntaxKind::ON {
                    // DISTINCTにONキーワードを追加
                    distinct_clause.extend_kw(cursor.node());

                    cursor.goto_next_sibling();

                    // 括弧と expr_list を ColumnList に格納
                    let mut column_list =
                        ColumnList::try_from(self.handle_parenthesized_expr_list(cursor, src)?)?;
                    // 改行によるフォーマットを強制
                    column_list.set_force_multi_line(true);

                    // ColumnListをSeparatedLinesに格納してBody
                    let mut sep_lines = SeparatedLines::new();

                    sep_lines.add_expr(
                        Expr::ColumnList(Box::new(column_list)).to_aligned(),
                        None,
                        vec![],
                    );

                    distinct_clause.set_body(Body::SepLines(sep_lines));
                }

                select_body.set_all_distinct(distinct_clause);

                cursor.goto_parent();
                // cursor -> distinct_clause
                ensure_kind!(cursor, SyntaxKind::distinct_clause, src);

                cursor.goto_next_sibling();
            }
            _ => {}
        }

        let extra_leading_comma = if cursor.node().kind() == SyntaxKind::Comma {
            cursor.goto_next_sibling();

            Some(COMMA.to_string())
        } else {
            None
        };

        self.consume_comments_in_clause(cursor, &mut clause)?;

        // cursor -> target_list?
        if cursor.node().kind() == SyntaxKind::target_list {
            let target_list = self.visit_target_list(cursor, src, extra_leading_comma)?;
            // select_clause_body 部分に target_list から生成した Body をセット
            select_body.set_select_clause_body(target_list);

            ensure_kind!(cursor, SyntaxKind::target_list, src);
        }

        clause.set_body(Body::Select(Box::new(select_body)));

        // フラットに並んでいるため goto_parent しない
        // cursor -> SELECT or target_list
        Ok(clause)
    }
}
