use postgresql_cst_parser::syntax_kind::SyntaxKind;
use tree_sitter::TreeCursor;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor, pg_ensure_kind, Visitor, COMMENT},
};

impl Visitor {
    /// SELECT文
    /// 呼び出し後、cursorはselect_statementを指す
    pub(crate) fn visit_select_stmt(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // SELECT文の定義
        // select_statement =
        //      [with_clause]
        //      select_clause (clause 自体はない)
        //      [from_clause]
        //      [where_clause]
        //      [_combining_query]
        //      [order_by_clause]
        //      [limit_clause]
        //      [offset_clause]

        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cusor -> with_clause?

        if cursor.node().kind() == SyntaxKind::with_clause {
            return Err(UroboroSQLFmtError::Unimplemented(
                "visit_select_stmt(): with_clause\n".to_string(),
            ));

            // // with句を追加する
            // let mut with_clause = self.visit_with_clause(cursor, src)?;
            // cursor.goto_next_sibling();
            // // with句の後に続くコメントを消費する
            // self.consume_comment_in_clause(cursor, src, &mut with_clause)?;

            // statement.add_clause(with_clause);
        }

        // cursor -> SELECT keyword
        // select_clause を消去したので、select_clause の中身が並ぶ
        pg_ensure_kind(cursor, SyntaxKind::SELECT, src)?;

        // select句を追加する
        statement.add_clause(self.visit_select_clause(cursor, src)?);

        // TODO: from句以下を追加する
        while cursor.goto_next_sibling() {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            // match cursor.node().kind() {
            //     "from_clause" => {
            //         let clause = self.visit_from_clause(cursor, src)?;
            //         statement.add_clause(clause);
            //     }
            //     // where_clause: $ => seq(kw("WHERE"), $._expression),
            //     "where_clause" => {
            //         let clause = self.visit_where_clause(cursor, src)?;
            //         statement.add_clause(clause);
            //     }
            //     "join_clause" => {
            //         let clauses = self.visit_join_clause(cursor, src)?;
            //         clauses.into_iter().for_each(|c| statement.add_clause(c));
            //     }
            //     "UNION" | "INTERSECT" | "EXCEPT" => {
            //         // 演算(e.g., "INTERSECT", "UNION ALL", ...)
            //         let mut combining_clause = Clause::from_node(cursor.node(), src);

            //         cursor.goto_next_sibling();
            //         // cursor -> (ALL | DISTINCT) | select_statement

            //         if matches!(cursor.node().kind(), "ALL" | "DISTINCT") {
            //             // ALL または DISTINCT を追加する
            //             combining_clause.extend_kw(cursor.node(), src);
            //             cursor.goto_next_sibling();
            //         }
            //         // cursor -> comments | select_statement

            //         // 演算子のみからなる句を追加
            //         statement.add_clause(combining_clause);

            //         while cursor.node().kind() == COMMENT {
            //             let comment = Comment::new(cursor.node(), src);
            //             statement.add_comment_to_child(comment)?;
            //             cursor.goto_next_sibling();
            //         }

            //         // 副問い合わせを計算
            //         let select_stmt = self.visit_select_stmt(cursor, src)?;
            //         select_stmt
            //             .get_clauses()
            //             .iter()
            //             .for_each(|clause| statement.add_clause(clause.to_owned()));

            //         // cursorはselect_statementになっているはずである
            //     }
            //     "group_by_clause" => {
            //         let clauses = self.visit_group_by_clause(cursor, src)?;
            //         clauses.into_iter().for_each(|c| statement.add_clause(c));
            //     }
            //     "order_by_clause" => {
            //         let clause = self.visit_order_by_clause(cursor, src)?;
            //         statement.add_clause(clause);
            //     }
            //     "limit_clause" => {
            //         let clause = self.visit_limit_clause(cursor, src)?;
            //         statement.add_clause(clause);
            //     }
            //     "offset_clause" => {
            //         let clause = self.visit_offset_clause(cursor, src)?;
            //         statement.add_clause(clause);
            //     }
            //     "for_update_clause" => {
            //         let clause = self.visit_for_update_clause(cursor, src)?;
            //         statement.add_clauses(clause);
            //     }
            //     COMMENT => {
            //         statement.add_comment_to_child(Comment::new(cursor.node(), src))?;
            //     }
            //     "ERROR" => {
            //         return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
            //             "visit_select_stmt: ERROR node appeared \n{}",
            //             error_annotation_from_cursor(cursor, src)
            //         )));
            //     }
            //     _ => {
            //         break;
            //     }
            // }
        }

        cursor.goto_parent();
        pg_ensure_kind(cursor, SyntaxKind::SelectStmt, src)?;

        Ok(statement)
    }
}
