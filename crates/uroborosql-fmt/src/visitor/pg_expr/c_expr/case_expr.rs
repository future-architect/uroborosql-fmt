use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::TreeCursor};

use crate::{
    cst::{Body, Comment, CondExpr, Location},
    error::UroboroSQLFmtError,
    util::convert_keyword_case,
    visitor::{pg_create_clause, pg_ensure_kind, pg_error_annotation_from_cursor},
};

use super::Visitor;

//
// case_expr の構造
// - CASE case_arg? when_clause_list case_default? END_P
//
// case_arg
// - a_expr
//
// when_clause_list
// - when_clause (when_clause)*
// フラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/12
//
// when_clause
// - WHEN a_expr THEN a_expr
//
// case_default
// - ELSE a_expr

impl Visitor {
    /// CASE 式を走査して CondExpr を返す
    /// 呼出し後、cursor は case_expr を指している
    pub fn visit_case_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<CondExpr, UroboroSQLFmtError> {
        // case_expr の構造
        // - CASE case_arg? when_clause_list case_default? END_P

        let mut cond_expr = CondExpr::new(Location::from(cursor.node().range()));

        cursor.goto_first_child();
        // cursor -> CASE

        let case_keyword = convert_keyword_case(cursor.node().text());
        cond_expr.set_case_keyword(&case_keyword);

        cursor.goto_next_sibling();

        // cursor -> Comment?
        // 最後のコメントは単純 CASE 式の条件部分に対するバインドパラメータの可能性があるので追加せず保持しておく
        let mut comments_after_case_keyword = Vec::new();
        while cursor.node().is_comment() {
            comments_after_case_keyword.push(Comment::pg_new(cursor.node()));
            cursor.goto_next_sibling();
        }

        // cursor -> case_arg?
        if cursor.node().kind() == SyntaxKind::case_arg {
            self.visit_case_arg(cursor, src, &mut cond_expr, comments_after_case_keyword)?;

            cursor.goto_next_sibling();
        } else {
            // case_arg がない場合、CASE キーワード後のコメントを追加する
            for comment in comments_after_case_keyword {
                cond_expr.set_trailing_comment(comment)?;
            }
        }

        // cursor -> Comment?
        // case_arg がある場合、その後のコメントを処理
        while cursor.node().is_comment() {
            cond_expr.set_trailing_comment(Comment::pg_new(cursor.node()))?;
            cursor.goto_next_sibling();
        }

        // cursor -> when_clause_list
        self.visit_when_clause_list(cursor, src, &mut cond_expr)?;
        cursor.goto_next_sibling();

        // cursor -> Comment?
        while cursor.node().is_comment() {
            cond_expr.set_trailing_comment(Comment::pg_new(cursor.node()))?;
            cursor.goto_next_sibling();
        }

        // cursor -> case_default?
        if cursor.node().kind() == SyntaxKind::case_default {
            self.visit_case_default(cursor, src, &mut cond_expr)?;

            cursor.goto_next_sibling();
        }

        // cursor -> Comment?
        while cursor.node().is_comment() {
            cond_expr.set_trailing_comment(Comment::pg_new(cursor.node()))?;
            cursor.goto_next_sibling();
        }

        // cursor -> END_P
        pg_ensure_kind!(cursor, SyntaxKind::END_P, src);
        let end_keyword = convert_keyword_case(cursor.node().text());
        cond_expr.set_end_keyword(&end_keyword);

        cursor.goto_parent();
        // cursor -> case_expr
        pg_ensure_kind!(cursor, SyntaxKind::case_expr, src);

        Ok(cond_expr)
    }

    fn visit_when_clause_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        cond_expr: &mut CondExpr,
    ) -> Result<(), UroboroSQLFmtError> {
        // when_clause_list
        // - when_clause (when_clause)*
        // フラット化されている: https://github.com/future-architect/postgresql-cst-parser/pull/12

        // cursor -> when_clause_list
        pg_ensure_kind!(cursor, SyntaxKind::when_clause_list, src);

        cursor.goto_first_child();
        // cursor -> when_clause
        self.visit_when_clause(cursor, src, cond_expr)?;

        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::when_clause => {
                    self.visit_when_clause(cursor, src, cond_expr)?;
                }
                SyntaxKind::C_COMMENT | SyntaxKind::SQL_COMMENT => {
                    cond_expr.set_trailing_comment(Comment::pg_new(cursor.node()))?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "case_expr: Unexpected syntax\n{}",
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> when_clause_list
        pg_ensure_kind!(cursor, SyntaxKind::when_clause_list, src);

        Ok(())
    }

    /// 引数に CondExpr を受け取り、case_arg を走査して expr を設定する
    /// 直前のコメントがあれば受け取り、それがバインドパラメータであれば式として処理する
    fn visit_case_arg(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        cond_expr: &mut CondExpr,
        comments_after_case_keyword: Vec<Comment>,
    ) -> Result<(), UroboroSQLFmtError> {
        // 単純CASE式
        cursor.goto_first_child();

        // cursor -> a_expr
        let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

        // 最後のコメントは単純CASE式の条件部分に対するバインドパラメータの可能性がある
        // バインドパラメータであれば式に付与し、それ以外のコメントは CondExpr に追加する
        if let Some((last, rest)) = comments_after_case_keyword.split_last() {
            // 最後以外のコメントは CondExpr に追加
            for comment in rest {
                cond_expr.set_trailing_comment(comment.clone())?;
            }

            // 最後のコメントが式に対するバインドパラメータならば式に付与し、
            // そうでなければ CondExpr に追加する
            if last.is_block_comment() && last.loc().is_next_to(&expr.loc()) {
                expr.set_head_comment(last.clone());
            } else {
                cond_expr.set_trailing_comment(last.clone())?;
            }
        }

        cond_expr.set_expr(expr);

        cursor.goto_parent();
        // cursor -> case_arg
        pg_ensure_kind!(cursor, SyntaxKind::case_arg, src);

        Ok(())
    }

    /// 引数に CondExpr を受け取り、when clause と then clause を追加する
    /// 呼出し後、cursor は when_clause を指している
    fn visit_when_clause(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        cond_expr: &mut CondExpr,
    ) -> Result<(), UroboroSQLFmtError> {
        // when_clause
        // - WHEN a_expr THEN a_expr

        // cursor -> when_clause
        pg_ensure_kind!(cursor, SyntaxKind::when_clause, src);

        cursor.goto_first_child();
        // cursor -> WHEN

        let mut when_clause = pg_create_clause!(cursor, SyntaxKind::WHEN);
        cursor.goto_next_sibling();
        // cursor -> Comment?
        self.pg_consume_comments_in_clause(cursor, &mut when_clause)?;

        // cursor -> a_expr
        let when_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        when_clause.set_body(Body::from(when_expr));

        cursor.goto_next_sibling();
        // cursor -> Comment?
        self.pg_consume_comments_in_clause(cursor, &mut when_clause)?;

        // cursor -> THEN
        let mut then_clause = pg_create_clause!(cursor, SyntaxKind::THEN);
        cursor.goto_next_sibling();
        // cursor -> Comment?
        self.pg_consume_comments_in_clause(cursor, &mut then_clause)?;

        // cursor -> a_expr
        let then_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        then_clause.set_body(Body::from(then_expr));

        cond_expr.add_when_then_clause(when_clause, then_clause);

        cursor.goto_parent();
        // cursor -> when_clause
        pg_ensure_kind!(cursor, SyntaxKind::when_clause, src);

        Ok(())
    }

    /// 引数に CondExpr を受け取り、case_default を走査して else 節を設定する
    fn visit_case_default(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        cond_expr: &mut CondExpr,
    ) -> Result<(), UroboroSQLFmtError> {
        // case_default
        // - ELSE a_expr

        // cursor -> case_default
        pg_ensure_kind!(cursor, SyntaxKind::case_default, src);

        cursor.goto_first_child();
        // cursor -> ELSE

        let mut else_clause = pg_create_clause!(cursor, SyntaxKind::ELSE);
        cursor.goto_next_sibling();
        // cursor -> Comment?
        self.pg_consume_comments_in_clause(cursor, &mut else_clause)?;

        // cursor -> a_expr
        let else_expr = self.visit_a_expr_or_b_expr(cursor, src)?;
        else_clause.set_body(Body::from(else_expr));

        cond_expr.set_else_clause(else_clause);

        cursor.goto_parent();
        // cursor -> case_default
        pg_ensure_kind!(cursor, SyntaxKind::case_default, src);

        Ok(())
    }
}
