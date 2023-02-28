use itertools::repeat_n;

use crate::cst::{Comment, Location, UroboroSQLFmtError};

use super::Expr;

#[derive(Debug, Clone)]
pub(crate) struct ParenExpr {
    depth: usize,
    expr: Expr,
    loc: Location,
    start_comments: Vec<Comment>,
    end_comments: Vec<Comment>,
}

impl ParenExpr {
    pub(crate) fn new(expr: Expr, loc: Location, depth: usize) -> ParenExpr {
        ParenExpr {
            depth,
            expr,
            loc,
            start_comments: vec![],
            end_comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if self.expr.loc().is_same_line(&comment.loc()) {
            self.expr.add_comment_to_child(comment)?;
        } else {
            self.add_end_comment(comment);
        }

        Ok(())
    }

    pub(crate) fn set_loc(&mut self, loc: Location) {
        self.loc = loc;
    }

    // 開きかっこから最初の式の間に現れるコメントを追加する
    pub(crate) fn add_start_comment(&mut self, comment: Comment) {
        self.start_comments.push(comment);
    }

    // 最後の式から閉じかっこの間に現れるコメントを追加する
    pub(crate) fn add_end_comment(&mut self, comment: Comment) {
        self.end_comments.push(comment);
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        for comment in &self.start_comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        let formatted = self.expr.render()?;

        // bodyでない式は、最初の行のインデントを自分で行わない。
        // そのため、かっこのインデントの深さ + 1個分インデントを挿入する。
        if !self.expr.is_body() {
            result.extend(repeat_n('\t', self.depth + 1));
        }

        result.push_str(&formatted);

        // インデント同様に、最後の改行も行う
        if !self.expr.is_body() {
            result.push('\n');
        }

        for comment in &self.end_comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        result.extend(repeat_n('\t', self.depth));
        result.push(')');
        Ok(result)
    }
}
