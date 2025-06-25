use postgresql_cst_parser::syntax_kind::SyntaxKind;
use postgresql_cst_parser::tree_sitter::TreeCursor;

use crate::{
    cst::{AlignedExpr, ColumnList, Comment, FunctionCallArgs, Location, SeparatedLines},
    error::UroboroSQLFmtError,
    new_visitor::pg_ensure_kind,
};

use super::{pg_error_annotation_from_cursor, Visitor};

#[derive(Debug, Clone)]
struct ExprListItem {
    expr: AlignedExpr,
    following_comments: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExprList {
    items: Vec<ExprListItem>,
}

impl ExprList {
    fn new() -> Self {
        Self { items: vec![] }
    }
    
    fn first_expr_mut(&mut self) -> Option<&mut AlignedExpr> {
        self.items.first_mut().map(|item| &mut item.expr)
    }

    fn add_expr(&mut self, expr: AlignedExpr) {
        self.items.push(ExprListItem {
            expr,
            following_comments: vec![],
        });
    }

    fn add_comment_to_last_item(&mut self, comment: Comment) -> Result<(), UroboroSQLFmtError> {
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
                "ExprList::add_comment_to_last_item(): Unexpected syntax. \n{}".to_string()
            ))
        }
    }
    
    pub(crate) fn to_separated_lines(&self, sep: Option<String>) -> SeparatedLines {
        let mut sep_lines = SeparatedLines::new();

        let Some((first, rest)) = self.items.split_first() else {
            return sep_lines;
        };

        let ExprListItem { expr, following_comments } = first;
        sep_lines.add_expr(expr.clone(), None, vec![]);

        for comment in following_comments {
            sep_lines.add_comment_to_child(comment.clone());
        }
        
        for item in rest {
            let ExprListItem { expr, following_comments } = item;

            sep_lines.add_expr(expr.clone(), sep.clone(), vec![]);

            for comment in following_comments {
                sep_lines.add_comment_to_child(comment.clone());
            }
        }

        sep_lines
    }
}

impl Visitor {
    /// 呼出し後、 cursor は expr_list を指している
    pub(crate) fn visit_expr_list(
        &mut self,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> Result<ExprList, UroboroSQLFmtError> {
        // cursor -> expr_list
        pg_ensure_kind!(cursor, SyntaxKind::expr_list, src);

        cursor.goto_first_child();
        // cursor -> a_expr

        let mut expr_list = ExprList::new();

        // 最初の要素
        if cursor.node().kind() == SyntaxKind::a_expr {
            expr_list.add_expr(self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned());
        }

        // 残りの要素
        // cursor -> a_expr | Comma | C_COMMENT | SQL_COMMENT
        while cursor.goto_next_sibling() {
            match cursor.node().kind() {
                SyntaxKind::Comma => {}
                SyntaxKind::a_expr => {
                    expr_list.add_expr(self.visit_a_expr_or_b_expr(cursor, src)?.to_aligned());
                }
                SyntaxKind::C_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());
                    
                    cursor.goto_next_sibling();

                    // cursor -> a_expr
                    pg_ensure_kind!(cursor, SyntaxKind::a_expr, src);
                    let mut expr = self.visit_a_expr_or_b_expr(cursor, src)?;

                    // コメントがバインドパラメータならば式に付与し、そうでなければすでにある式の下に追加
                    if comment.is_block_comment() && comment.loc().is_next_to(&expr.loc()) {
                        expr.set_head_comment(comment.clone());
                    } else {
                        expr_list.add_comment_to_last_item(comment)?;
                    }

                    expr_list.add_expr(expr.to_aligned());
                }
                SyntaxKind::SQL_COMMENT => {
                    let comment = Comment::pg_new(cursor.node());

                    expr_list.add_comment_to_last_item(comment)?;
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

        Ok(expr_list)
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
        // いづれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for (index, item) in paren_list.expr_list.items.iter().enumerate() {
            if !item.following_comments.is_empty() {
                return Err(UroboroSQLFmtError::Unimplemented(
                    format!("Comments following column list at position {} are not supported", index + 1)
                ));
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
        
        // いづれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for (index, item) in paren_list.expr_list.items.iter().enumerate() {
            if !item.following_comments.is_empty() {
                return Err(UroboroSQLFmtError::Unimplemented(
                    format!("Comments following function argument at position {} are not supported", index + 1)
                ));
            }
            exprs.push(item.expr.clone());
        }

        Ok(FunctionCallArgs::new(exprs, paren_list.location))
    }
}

impl Visitor {
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
            let comment = Comment::pg_new(cursor.node());
            expr_list.add_comment_to_last_item(comment)?;

            cursor.goto_next_sibling();
        }

        // cursor -> ')'
        pg_ensure_kind!(cursor, SyntaxKind::RParen, src);
        // Location が括弧全体を指すよう更新
        loc.append(Location::from(cursor.node().range()));

        Ok(ParenthesizedExprList::new(expr_list, loc, start_comments))
    }
}
