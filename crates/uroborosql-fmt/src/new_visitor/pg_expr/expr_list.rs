use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, ColumnList, Comment, FunctionCallArgs, Location},
    error::UroboroSQLFmtError,
    new_visitor::pg_ensure_kind,
};

use super::{pg_error_annotation_from_cursor, Visitor};

/// 括弧で囲まれた式リストの共通表現
#[derive(Debug, Clone)]
pub struct ParenthesizedExprList {
    pub exprs: Vec<AlignedExpr>,
    pub location: Location,
    pub start_comments: Vec<Comment>,
}

impl ParenthesizedExprList {
    pub fn new(exprs: Vec<AlignedExpr>, location: Location, start_comments: Vec<Comment>) -> Self {
        Self {
            exprs,
            location,
            start_comments,
        }
    }
}

impl From<ParenthesizedExprList> for ColumnList {
    fn from(paren_list: ParenthesizedExprList) -> Self {
        ColumnList::new(
            paren_list.exprs,
            paren_list.location,
            paren_list.start_comments,
        )
    }
}

/// FunctionCallArgsへの変換
impl TryFrom<ParenthesizedExprList> for FunctionCallArgs {
    type Error = UroboroSQLFmtError;

    fn try_from(paren_list: ParenthesizedExprList) -> Result<Self, Self::Error> {
        if !paren_list.start_comments.is_empty() {
            return Err(UroboroSQLFmtError::Unimplemented(
                "Comments immediately after opening parenthesis in function arguments are not supported".to_string()
            ));
        }
        Ok(FunctionCallArgs::new(paren_list.exprs, paren_list.location))
    }
}

impl Visitor {
    /// 呼出し後、 cursor は expr_list を指している
    pub(crate) fn visit_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<Vec<AlignedExpr>, UroboroSQLFmtError> {
        // cursor -> expr_list
        pg_ensure_kind!(cursor, SyntaxKind::expr_list, src);

        cursor.goto_first_child();
        // cursor -> a_expr

        let mut exprs = Vec::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            exprs.push(self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned());
        }

        // 残りの要素
        // cursor -> a_expr | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    exprs.push(self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned());
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
                    pg_ensure_kind!(cursor, SyntaxKind::a_expr, src);
                    let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

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
    /// 呼出し時、cursor は '(' を指している
    /// 呼出し後、cursor は ')' を指している
    pub(crate) fn handle_parenthesized_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ParenthesizedExprList, UroboroSQLFmtError> {
        // cursor -> '('
        pg_ensure_kind!(cursor, SyntaxKind::LParen, src);
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
        pg_ensure_kind!(cursor, SyntaxKind::expr_list, src);
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
                    "handle_parenthesized_expr_list(): Unexpected comment\n{}",
                    pg_error_annotation_from_cursor(cursor, src)
                )));
            }

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        // Location が括弧全体を指すよう更新
        loc.append(Location::from(cursor.node().range()));

        Ok(ParenthesizedExprList::new(exprs, loc, start_comments))
    }
}
