mod a_expr;
mod c_expr;

use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, ColumnList, Comment, Expr, Location},
    error::UroboroSQLFmtError,
};

use super::{pg_ensure_kind, pg_error_annotation_from_cursor, Visitor};
pub(crate) enum AExprOrBExpr {
    AExpr,
    BExpr,
}

impl AExprOrBExpr {
    pub(crate) fn kind(&self) -> SyntaxKind {
        match self {
            AExprOrBExpr::AExpr => SyntaxKind::a_expr,
            AExprOrBExpr::BExpr => SyntaxKind::b_expr,
        }
    }
}

impl Visitor {
    /// a_expr または b_expr を走査する
    /// 引数に a_expr か b_expr のどちらを走査するかを指定する
    /// 呼出し後、cursor は呼出し時の位置に戻る
    pub(crate) fn visit_a_expr_or_b_expr(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
        expr_type: AExprOrBExpr,
    ) -> Result<Expr, UroboroSQLFmtError> {
        // b_expr は a_expr のサブセットであるため、a_expr および b_expr の走査には a_expr 用の visitor をそのまま使う

        cursor.goto_first_child();
        // cursor -> c_expr | DEFAULT | Plus | Minus | NOT | qual_Op | a_expr | UNIQUE
        let expr = self.handle_a_expr_inner(cursor, src)?;

        // cursor -> (last_node)
        assert!(!cursor.goto_next_sibling());

        cursor.goto_parent();
        // cursor -> a_expr or b_expr (parent)
        pg_ensure_kind(cursor, expr_type.kind(), src)?;

        Ok(expr)
    }

    /// 呼出し後、 cursor は expr_list を指している
    pub(crate) fn visit_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // cursor -> expr_list
        pg_ensure_kind(cursor, SyntaxKind::expr_list, src)?;

        cursor.goto_first_child();
        // cursor -> a_expr

        let mut exprs = Vec::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            exprs.push(
                self.visit_a_expr_or_b_expr(cursor, src, AExprOrBExpr::AExpr)?
                    .to_aligned(),
            );
        }

        // 残りの要素
        // cursor -> a_expr | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    exprs.push(
                        self.visit_a_expr_or_b_expr(cursor, src, AExprOrBExpr::AExpr)?
                            .to_aligned(),
                    );
                }
                // バインドパラメータを想定
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // 次の式へ
                    if !cursor.goto_next_sibling() {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                            "visit_expr_list(): Unexpected syntax. node: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    // cursor -> a_expr
                    pg_ensure_kind(cursor, SyntaxKind::a_expr, src)?;
                    let mut expr = self.visit_a_expr_or_b_expr(cursor, src, AExprOrBExpr::AExpr)?;

                    // コメントがバインドパラメータならば式に付与
                    if comment.is_block_comment() && comment.loc().is_next_to(&expr.loc()) {
                        expr.set_head_comment(comment.clone());
                    } else {
                        // バインドパラメータでないブロックコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_expr_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }

                    exprs.push(expr.to_aligned());
                }
                // 行末コメント
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    // exprs は必ず1つ以上要素を持っている
                    let last = exprs.last_mut().unwrap();
                    if last.loc().is_same_line(&comment.loc()) {
                        last.set_trailing_comment(comment)?;
                    } else {
                        // 行末コメント以外のコメントは想定していない
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "visit_expr_list(): Unexpected comment\nnode_kind: {}\n{}",
                            cursor.node().kind(),
                            pg_error_annotation_from_cursor(cursor, src)
                        )));
                    }
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_expr_list(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        pg_error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> expr_list

        Ok(exprs)
    }

    /// 括弧で囲まれた式リストを処理するメソッド
    pub(crate) fn visit_parenthesized_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // cursor -> '('
        pg_ensure_kind(cursor, SyntaxKind::LParen, src)?;
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> comment?

        // 開き括弧と式との間にあるコメントを保持
        // 最後の要素はバインドパラメータの可能性があるので、最初の式を処理した後で付け替える
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::pg_new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr_list
        pg_ensure_kind(cursor, SyntaxKind::expr_list, src)?;
        let mut exprs = self.visit_expr_list(cursor, src)?;

        // start_comments のうち最後のものは、 expr_list の最初の要素のバインドパラメータの可能性がある
        if let Some(comment) = start_comments.last() {
            // 定義上、 expr_list は必ず一つ以上の要素を持つ
            let first_expr = exprs.first_mut().unwrap();

            if comment.is_block_comment() && comment.loc().is_next_to(&first_expr.loc()) {
                // ブロックコメントかつ式に隣接していればバインドパラメータなので、式に付与する
                first_expr.set_head_comment(comment.clone());

                // start_comments からも削除
                start_comments.pop().unwrap();
            }
        }

        cursor.goto_next_sibling();
        // cursor -> comment?

        if cursor.node().is_comment() {
            // 行末コメントを想定する
            let comment = Comment::pg_new(cursor.node());

            // exprs は必ず1つ以上要素を持っている
            let last = exprs.last_mut().unwrap();
            if last.loc().is_same_line(&comment.loc()) {
                last.set_trailing_comment(comment)?;
            } else {
                // 行末コメント以外のコメントは想定していない
                return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                    "visit_parenthesized_expr_list(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind(cursor, SyntaxKind::RParen, src)?;
        // Location が括弧全体を指すよう更新
        loc.append(Location::from(cursor.node().range()));

        Ok(ColumnList::new(exprs, loc, start_comments))
    }
}
