use crate::{
    cst::{AlignedExpr, Comment, Expr, Location},
    error::UroboroSQLFmtError,
};

#[derive(Debug, Clone)]
pub(crate) struct SingleLine {
    expr: AlignedExpr,
    loc: Location,
    comments: Vec<Comment>,
}

impl SingleLine {
    pub(crate) fn new(expr: Expr) -> SingleLine {
        let expr = expr.to_aligned();
        let loc = expr.loc();
        SingleLine {
            expr,
            loc,
            comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() || !self.loc.is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            self.comments.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.expr.set_trailing_comment(comment)?;
        }
        Ok(())
    }

    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        if comment.loc().is_next_to(&self.expr.loc()) {
            self.expr.set_head_comment(comment);
            true
        } else {
            false
        }
    }

    /// 先頭にインデントを挿入せずに render する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 式は一つのみであるため、縦ぞろえはしない
        result.push_str(&self.expr.render(depth)?);

        result.push('\n');
        if !self.comments.is_empty() {
            result.push_str(
                &self
                    .comments
                    .iter()
                    .map(|c| c.render(depth))
                    .collect::<Result<Vec<_>, _>>()?
                    .join("\n"),
            );
            result.push('\n');
        }

        Ok(result)
    }
}
