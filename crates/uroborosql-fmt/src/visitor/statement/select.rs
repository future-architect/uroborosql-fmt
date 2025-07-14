use postgresql_cst_parser::syntax_kind::SyntaxKind;

use crate::{
    cst::*,
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, error_annotation_from_cursor, Visitor},
};

/// SelectStmt を visit した結果取りうるパターンを表す型
/// cst モジュールの構造体に格納するまでに使う中間表現
pub(crate) enum SelectStmtOutput {
    Statement(Statement),
    /// SubExpr や ParenExpr が対応
    Expr(Expr),
    /// VALUES句のキーワードと本体
    Values(String, Vec<ColumnList>),
}

impl Visitor {
    /// SelectStmt をフォーマットする
    pub(crate) fn visit_select_stmt(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<SelectStmtOutput, UroboroSQLFmtError> {
        // SelectStmt
        // ├ select_no_parens
        // │  ├ (simple_select)
        // │  │  ├ SELECT opt_all_clause opt_target_list into_clause from_clause where_clause group_clause having_clause window_clause
        // │  │  ├ SELECT distinct_clause target_list into_clause from_clause where_clause group_clause having_clause window_clause
        // │  │  ├ values_clause
        // │  │  ├ TABLE relation_expr
        // │  │  ├ (select_clause) UNION set_quantifier (select_clause)
        // │  │  ├ (select_clause) INTERSECT set_quantifier (select_clause)
        // │  │  └ (select_clause) EXCEPT set_quantifier (select_clause)
        // │  ├ (select_clause) sort_clause
        // │  ├ (select_clause) (opt_sort_clause) for_locking_clause (opt_select_limit)
        // │  ├ (select_clause) (opt_sort_clause)  (select_limit) opt_for_locking_clause
        // │  ├ with_clause (select_clause)
        // │  ├ with_clause (select_clause) sort_clause
        // │  ├ with_clause (select_clause) (opt_sort_clause) for_locking_clause (select_limit)
        // │  └ with_clause (select_clause) (opt_sort_clause) (select_limit) opt_for_locking_clause
        // └ select_with_parens
        //    ├ '(' select_no_parens ')'
        //    └ '(' select_with_parens ')'
        //
        // 括弧で囲まれているノードは PostgreSQL の文法定義上は存在するが、 postgresql-cst-parser が返す木では削除されるため登場しない
        // 整理のためこのように表記しているが、実際には子ノードが同じレベルに展開される
        //

        cursor.goto_first_child();
        // cursor -> select_no_parens | select_with_parens
        ensure_kind!(
            cursor,
            SyntaxKind::select_no_parens | SyntaxKind::select_with_parens,
            src
        );

        let result = match cursor.node().kind() {
            SyntaxKind::select_no_parens => self.visit_select_no_parens(cursor, src)?,
            SyntaxKind::select_with_parens => {
                let expr = self.visit_select_with_parens(cursor, src)?;
                SelectStmtOutput::Expr(expr)
            }
            _ => {
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_select_stmt(): {} node appeared. This node is not considered yet.\n{}",
                    cursor.node().kind(),
                    error_annotation_from_cursor(cursor, src)
                )));
            }
        };

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::SelectStmt, src);

        Ok(result)
    }

    pub(crate) fn visit_select_no_parens(
        &mut self,
        cursor: &mut postgresql_cst_parser::tree_sitter::TreeCursor,
        src: &str,
    ) -> Result<SelectStmtOutput, UroboroSQLFmtError> {
        // select_no_parens
        //  ├ (simple_select)
        //  │  ├ SELECT opt_all_clause opt_target_list into_clause from_clause where_clause group_clause having_clause window_clause
        //  │  ├ SELECT distinct_clause target_list into_clause from_clause where_clause group_clause having_clause window_clause
        //  │  ├ values_clause
        //  │  ├ TABLE relation_expr
        //  │  ├ (select_clause) UNION set_quantifier (select_clause)
        //  │  ├ (select_clause) INTERSECT set_quantifier (select_clause)
        //  │  └ (select_clause) EXCEPT set_quantifier (select_clause)
        //  ├ (select_clause) sort_clause
        //  ├ (select_clause) (opt_sort_clause) for_locking_clause (opt_select_limit)
        //  ├ (select_clause) (opt_sort_clause)  (select_limit) opt_for_locking_clause
        //  ├ with_clause (select_clause)
        //  ├ with_clause (select_clause) sort_clause
        //  ├ with_clause (select_clause) (opt_sort_clause) for_locking_clause (select_limit)
        //  └ with_clause (select_clause) (opt_sort_clause) (select_limit) opt_for_locking_clause

        // (select_clause)
        //  ├ (simple_select)
        //  └ select_with_parens

        let mut statement = Statement::new();

        cursor.goto_first_child();

        // cursor -> values_clause | select_with_parens | TABLE | with_clause | SELECT

        // cursor -> values_clause?
        if cursor.node().kind() == SyntaxKind::values_clause {
            let (values_keyword, rows) = self.visit_values_clause(cursor, src)?;

            // values_clause の場合は Statement 中にこれ以上の要素が現れない
            assert!(
                !cursor.goto_next_sibling(),
                "unexpected node after values_clause in Statement"
            );

            cursor.goto_parent();
            ensure_kind!(cursor, SyntaxKind::select_no_parens, src);

            return Ok(SelectStmtOutput::Values(values_keyword, rows));
        }

        // cursor -> select_with_parens
        if cursor.node().kind() == SyntaxKind::select_with_parens {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_select_no_parens(): select_with_parens node appeared. This node is not considered yet.\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> TABLE
        if cursor.node().kind() == SyntaxKind::TABLE {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_select_no_parens(): TABLE node appeared. This node is not considered yet.\n{}",
                error_annotation_from_cursor(cursor, src)
            )));
        }

        // cursor -> with_clause?
        if cursor.node().kind() == SyntaxKind::with_clause {
            let mut with_clause = self.visit_with_clause(cursor, src)?;
            cursor.goto_next_sibling();

            // with句の後に続くコメントを消費する
            self.consume_comments_in_clause(cursor, &mut with_clause)?;

            statement.add_clause(with_clause);
        }

        // cursor -> SELECT keyword
        // select_clause を消去したので、select_clause の中身が並ぶ
        ensure_kind!(cursor, SyntaxKind::SELECT, src);

        // select句を追加する
        statement.add_clause(self.visit_select_clause(cursor, src)?);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                // 現時点で考慮している構造
                // - into_clause
                // - from_clause
                // - where_clause
                // - group_clause
                // - having_clause
                // - window_clause
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                SyntaxKind::UNION | SyntaxKind::INTERSECT | SyntaxKind::EXCEPT => {
                    // (UNION | INTERSECT | EXCEPT) set_quantifier? (select_clause)

                    let mut combining_clause = Clause::from_node(cursor.node());
                    cursor.goto_next_sibling();

                    // cursor -> set_quantifier?
                    if cursor.node().kind() == SyntaxKind::set_quantifier {
                        // set_quantifier
                        // - ALL
                        // - DISTINCT
                        combining_clause.extend_kw(cursor.node());
                        cursor.goto_next_sibling();
                    }

                    // 演算子のみからなる句を追加
                    statement.add_clause(combining_clause);

                    while cursor.node().is_comment() {
                        let comment = Comment::new(cursor.node());
                        statement.add_comment_to_child(comment)?;
                        cursor.goto_next_sibling();
                    }

                    // cursor -> (select_clause)
                    let select_clause = self.visit_select_clause(cursor, src)?;
                    statement.add_clause(select_clause);
                }
                SyntaxKind::into_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): into_clause is not implemented\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::from_clause => {
                    let from_clause = self.visit_from_clause(cursor, src)?;
                    statement.add_clause(from_clause);
                }
                SyntaxKind::where_clause => {
                    let where_clause = self.visit_where_clause(cursor, src)?;
                    statement.add_clause(where_clause);
                }
                SyntaxKind::group_clause => {
                    let group_clause = self.visit_group_clause(cursor, src)?;
                    statement.add_clause(group_clause);
                }
                SyntaxKind::having_clause => {
                    let having_clause = self.visit_having_clause(cursor, src)?;
                    statement.add_clause(having_clause);
                }
                SyntaxKind::window_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): window_clause is not implemented\n{}",
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::sort_clause => {
                    let sort_clause = self.visit_sort_clause(cursor, src)?;
                    statement.add_clause(sort_clause);
                }
                SyntaxKind::limit_clause => {
                    let limit_clause = self.visit_limit_clause(cursor, src)?;
                    statement.add_clause(limit_clause);
                }
                SyntaxKind::offset_clause => {
                    let offset_clause = self.visit_offset_clause(cursor, src)?;
                    statement.add_clause(offset_clause);
                }
                SyntaxKind::for_locking_clause => {
                    let for_locking_clauses = self.visit_for_locking_clause(cursor, src)?;
                    statement.add_clauses(for_locking_clauses);
                }
                _ => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_select_stmt(): {} node appeared. This node is not considered yet.\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        ensure_kind!(cursor, SyntaxKind::select_no_parens, src);

        Ok(SelectStmtOutput::Statement(statement))
    }
}
