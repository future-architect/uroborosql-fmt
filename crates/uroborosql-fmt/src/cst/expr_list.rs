use crate::{
    cst::{AlignedExpr, ColumnList, Comment, FunctionCallArgs, Location, SeparatedLines},
    error::UroboroSQLFmtError,
};

#[derive(Debug, Clone)]
struct ExprListItem {
    sep: Option<String>,
    expr: AlignedExpr,
    following_comments: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExprList {
    items: Vec<ExprListItem>,
}

impl ExprList {
    pub(crate) fn new() -> Self {
        Self { items: vec![] }
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

    pub(crate) fn to_separated_lines(&self) -> Result<SeparatedLines, UroboroSQLFmtError> {
        let mut sep_lines = SeparatedLines::new();

        for item in &self.items {
            let ExprListItem {
                sep,
                expr,
                following_comments,
            } = item;

            sep_lines.add_expr(expr.clone(), sep.clone(), vec![]);

            for comment in following_comments {
                sep_lines.add_comment_to_child(comment.clone())?;
            }
        }

        Ok(sep_lines)
    }

    pub(crate) fn to_function_call_args(
        &self,
        location: Location,
    ) -> Result<FunctionCallArgs, UroboroSQLFmtError> {
        let mut exprs = Vec::new();
        for item in &self.items {
            if let Some(following_comment) = item.following_comments.first() {
                return Err(UroboroSQLFmtError::Unimplemented(
                    format!(
                        "Comments following function arguments are not supported. Only trailing comments are supported.\ncomment: {}",
                        following_comment.text()
                    ),
                ));
            }

            exprs.push(item.expr.clone());
        }

        Ok(FunctionCallArgs::new(exprs, location))
    }

    pub(crate) fn to_column_list(
        &self,
        location: Location,
        start_comments: Vec<Comment>,
    ) -> Result<ColumnList, UroboroSQLFmtError> {
        // いずれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for item in &self.items {
            if let Some(following_comment) = item.following_comments.first() {
                return Err(UroboroSQLFmtError::Unimplemented(
                    format!(
                        "Comments following columns are not supported. Only trailing comments are supported.\ncomment: {}",
                        following_comment.text()
                    ),
                ));
            }

            exprs.push(item.expr.clone());
        }

        Ok(ColumnList::new(exprs, location, start_comments))
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

impl TryFrom<ParenthesizedExprList> for ColumnList {
    type Error = UroboroSQLFmtError;

    fn try_from(paren_list: ParenthesizedExprList) -> Result<Self, Self::Error> {
        // いずれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for item in paren_list.expr_list.items {
            if let Some(following_comment) = item.following_comments.first() {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "Comments following columns are not supported. Only trailing comments are supported.\ncomment: {}",
                    following_comment.text()
                )));
            }
            exprs.push(item.expr.clone());
        }

        Ok(ColumnList::new(
            exprs,
            paren_list.location,
            paren_list.start_comments,
        ))
    }
}

impl TryFrom<ParenthesizedExprList> for FunctionCallArgs {
    type Error = UroboroSQLFmtError;

    fn try_from(paren_list: ParenthesizedExprList) -> Result<Self, Self::Error> {
        if !paren_list.start_comments.is_empty() {
            return Err(UroboroSQLFmtError::Unimplemented(
                "Comments immediately after opening parenthesis in function arguments are not supported".to_string()
            ));
        }

        // いずれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for item in paren_list.expr_list.items {
            if let Some(following_comment) = item.following_comments.first() {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "Comments following function arguments are not supported. Only trailing comments are supported.\ncomment: {}",
                    following_comment.text()
                )));
            }
            exprs.push(item.expr.clone());
        }

        Ok(FunctionCallArgs::new(exprs, paren_list.location))
    }
}
