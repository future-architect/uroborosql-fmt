use crate::{
    cst::{AlignedExpr, Comment, Location},
    error::UroboroSQLFmtError,
};

#[derive(Debug, Clone)]
pub(crate) struct ExprListItem {
    sep: Option<String>,
    expr: AlignedExpr,
    following_comments: Vec<Comment>,
}

impl ExprListItem {
    pub(crate) fn sep(&self) -> &Option<String> {
        &self.sep
    }

    pub(crate) fn expr(&self) -> &AlignedExpr {
        &self.expr
    }

    pub(crate) fn following_comments(&self) -> &Vec<Comment> {
        &self.following_comments
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ExprList {
    items: Vec<ExprListItem>,
}

impl ExprList {
    pub(crate) fn new() -> Self {
        Self { items: vec![] }
    }

    pub(crate) fn items(&self) -> &Vec<ExprListItem> {
        &self.items
    }

    pub(crate) fn first_expr_mut(&mut self) -> Option<&mut AlignedExpr> {
        self.items.first_mut().map(|item| &mut item.expr)
    }

    pub(crate) fn add_expr(&mut self, expr: AlignedExpr, sep: Option<String>) {
        self.items.push(ExprListItem {
            sep,
            expr,
            following_comments: vec![],
        });
    }

    pub(crate) fn add_comment_to_last_item(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if let Some(last) = self.items.last_mut() {
            // 行末コメントならば最後の式に追加
            if !comment.is_block_comment() && last.expr.loc().is_same_line(&comment.loc()) {
                last.expr.set_trailing_comment(comment)?;
            } else {
                // 行末コメントでなければ式の下に追加する
                last.following_comments.push(comment);
            }

            Ok(())
        } else {
            // 式がない場合はエラー
            Err(UroboroSQLFmtError::IllegalOperation(
                "ExprList::add_comment_to_last_item(): No expression to add comment to."
                    .to_string(),
            ))
        }
    }
}

/// 括弧で囲まれた式リストの共通表現
#[derive(Debug, Clone)]
pub struct ParenthesizedExprList {
    pub expr_list: ExprList,
    pub location: Location,
    pub start_comments: Vec<Comment>,
}

impl ParenthesizedExprList {
    pub fn new(expr_list: ExprList, location: Location, start_comments: Vec<Comment>) -> Self {
        Self {
            expr_list,
            location,
            start_comments,
        }
    }
}
