use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Comment, Expr, PrimaryExpr, PrimaryExprKind, SeparatedLines, Statement},
    error::UroboroSQLFmtError,
    new_visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
    util::convert_keyword_case,
    NewVisitor as Visitor, CONFIG,
};

// DeleteStmt
// - opt_with_clause? DELETE_P FROM relation_expr_opt_alias using_clause? where_or_current_clause? returning_clause?
//
// opt_with_clause
// - with_clause
//
// relation_expr_opt_alias
// - relation_expr AS? ColId?
//
// using_clause
// - USING from_list
//
// returning_clause
// - RETURNING returning_with_clause target_list
//
// returning_with_clause
// - WITH ( returning_options )
//
// returning_options
// - NEEDS_FLATTEN
// - returning_option (, returning_option )*
//
// returning_option
// - returning_option_kind AS ColId
//
// returning_option_kind
// - OLD
// - NEW

impl Visitor {
    pub(crate) fn visit_delete_stmt(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Statement, UroboroSQLFmtError> {
        // DeleteStmt
        // - opt_with_clause? DELETE_P FROM relation_expr_opt_alias using_clause? where_or_current_clause? returning_clause?
        let mut statement = Statement::new();

        cursor.goto_first_child();
        // cursor -> opt_with_clause?

        if cursor.node().kind() == SyntaxKind::opt_with_clause {
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "visit_delete_stmt(): opt_with_clause is not implemented.\n{}",
                pg_error_annotation_from_cursor(cursor, src)
            )));

            // let with_clause = self.visit_with_clause(cursor, src)?;
            // statement.add_clause(with_clause);
            // self.pg_consume_comments_in_clause(cursor, src, &mut with_clause)?;
            // statement.add_clause(with_clause);
        }

        // cursor -> DELETE_P
        pg_ensure_kind!(cursor, SyntaxKind::DELETE_P, src);
        let mut clause = pg_create_clause!(cursor, SyntaxKind::DELETE_P);

        cursor.goto_next_sibling();
        self.pg_consume_or_complement_sql_id(cursor, &mut clause);
        self.pg_consume_comments_in_clause(cursor, &mut clause)?;

        statement.add_clause(clause);

        // cursor -> FROM
        let mut from_clause = pg_create_clause!(cursor, SyntaxKind::FROM);
        cursor.goto_next_sibling();

        // cursor -> relation_expr_opt_alias
        let body = self.visit_relation_expr_opt_alias(cursor, src)?;

        from_clause.set_body(body);
        statement.add_clause(from_clause);

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::using_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_delete_stmt(): using_clause is not implemented.\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::where_or_current_clause => {
                    let where_or_current_clause =
                        self.visit_where_or_current_clause(cursor, src)?;
                    statement.add_clause(where_or_current_clause);
                }
                SyntaxKind::returning_clause => {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "visit_delete_stmt(): returning_clause is not implemented.\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
                SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    statement.add_comment_to_child(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_delete_stmt(): unexpected node kind\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> DeleteStmt
        pg_ensure_kind!(cursor, SyntaxKind::DeleteStmt, src);

        Ok(statement)
    }

    fn visit_relation_expr_opt_alias(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Body, UroboroSQLFmtError> {
        // relation_expr_opt_alias
        // - relation_expr AS? ColId?

        cursor.goto_first_child();

        let relation_expr = self.visit_relation_expr(cursor, src)?;

        cursor.goto_next_sibling();

        // cursor -> AS?
        let as_keyword = if cursor.node().kind() == SyntaxKind::AS {
            let as_keyword = cursor.node().text().to_string();
            cursor.goto_next_sibling();

            Some(as_keyword)
        } else {
            None
        };

        let mut aligned = relation_expr.to_aligned();
        // cursor -> ColId?
        if cursor.node().kind() == SyntaxKind::ColId {
            let col_id = PrimaryExpr::with_pg_node(cursor.node(), PrimaryExprKind::Expr)?;
            let rhs = Expr::Primary(Box::new(col_id));

            // AS があり、かつ AS を除去する設定が有効ならば AS を除去する
            if let Some(as_keyword) = as_keyword {
                if CONFIG.read().unwrap().remove_table_as_keyword {
                    aligned.add_rhs(None, rhs);
                } else {
                    aligned.add_rhs(Some(convert_keyword_case(&as_keyword)), rhs);
                }
            } else {
                // AS がない場合はそのまま追加
                aligned.add_rhs(None, rhs);
            }
        };

        cursor.goto_parent();
        // cursor -> relation_expr_opt_alias
        pg_ensure_kind!(cursor, SyntaxKind::relation_expr_opt_alias, src);

        let mut body = SeparatedLines::new();
        body.add_expr(aligned, None, vec![]);

        Ok(Body::SepLines(body))
    }
}
