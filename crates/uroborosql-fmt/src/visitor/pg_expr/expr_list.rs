use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{Comment, ExprList, Location, ParenthesizedExprList},
    error::UroboroSQLFmtError,
    visitor::{ensure_kind, COMMA},
};

use super::{error_annotation_from_cursor, Visitor};

impl Visitor {
    /// 呼出し後、 cursor は expr_list を指している
    pub(crate) fn visit_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ExprList, UroboroSQLFmtError> {
        // cursor -> expr_list
        ensure_kind!(cursor, SyntaxKind::expr_list, src);

        cursor.goto_first_child();
        // cursor -> a_expr

        let mut expr_list = ExprList::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            expr_list.add_expr(self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned(), None);
        }

        // 残りの要素
        // cursor -> a_expr | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    expr_list.add_expr(
                        self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned(),
                        Some(COMMA.to_string()),
                    );
                }
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    cursor.goto_next_sibling();

                    // cursor -> a_expr
                    if cursor.node().kind() == SyntaxKind::a_expr {
                        let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

                        // コメントがバインドパラメータならば式に付与し、そうでなければすでにある式の下に追加
                        if comment.loc().is_next_to(&expr.loc()) {
                            expr.set_head_comment(comment.clone());
                        } else {
                            expr_list.add_comment_to_last_item(comment)?;
                        }

                        expr_list.add_expr(expr.to_aligned(), Some(COMMA.to_string()));
                    } else {
                        // 次に式が無い場合
                        expr_list.add_comment_to_last_item(comment)?;
                    }
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::new(cursor.node());

                    expr_list.add_comment_to_last_item(comment)?;
                }
                _ => {
                    return Err(UroboroSQLFmtError::UnexpectedSyntax(format!(
                        "visit_expr_list(): Unexpected syntax. node: {}\n{}",
                        cursor.node().kind(),
                        error_annotation_from_cursor(cursor, src)
                    )));
                }
            }
        }

        cursor.goto_parent();
        // cursor -> expr_list

        Ok(expr_list)
    }

    /// 括弧で囲まれた式リストを処理するメソッド
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    pub(crate) fn handle_parenthesized_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenthesizedExprList, UroboroSQLFmtError> {
        // cursor -> '('
        ensure_kind!(cursor, SyntaxKind::LParen, src);
        let mut loc = Location::from(cursor.node().range());

        cursor.goto_next_sibling();
        // cursor -> comment?

        // 開き括弧と式との間にあるコメントを保持
        // 最後の要素はバインドパラメータの可能性があるので、最初の式を処理した後で付け替える
        let mut start_comments = vec![];
        while cursor.node().is_comment() {
            let comment = Comment::new(cursor.node());
            start_comments.push(comment);
            cursor.goto_next_sibling();
        }

        // cursor -> expr_list
        ensure_kind!(cursor, SyntaxKind::expr_list, src);
        let mut expr_list = self.visit_expr_list(cursor, src)?;

        // start_comments のうち最後のものは、 expr_list の最初の要素のバインドパラメータの可能性がある
        if let Some(comment) = start_comments.last() {
            // 定義上、 expr_list は必ず一つ以上の要素を持つ
            let first_expr = expr_list.first_expr_mut().unwrap();

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
            let comment = Comment::new(cursor.node());
            expr_list.add_comment_to_last_item(comment)?;

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        ensure_kind!(cursor, SyntaxKind::RParen, src);
        // Location が括弧全体を指すよう更新
        loc.append(Location::from(cursor.node().range()));

        Ok(ParenthesizedExprList::new(expr_list, loc, start_comments))
    }
}
