use itertools::repeat_n;

use crate::{
    cst::{Comment, Location},
    error::UroboroSQLFmtError,
};

use super::Expr;

#[derive(Debug, Clone)]
pub(crate) struct ParenExpr {
    expr: Expr,
    loc: Location,
    start_comments: Vec<Comment>,
    end_comments: Vec<Comment>,
}

impl ParenExpr {
    pub(crate) fn new(expr: Expr, loc: Location) -> ParenExpr {
        ParenExpr {
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

    /// 開きかっこから最初の式の間に現れるコメントを追加する
    pub(crate) fn add_start_comment(&mut self, comment: Comment) {
        self.start_comments.push(comment);
    }

    /// 最後の式から閉じかっこの間に現れるコメントを追加する
    pub(crate) fn add_end_comment(&mut self, comment: Comment) {
        self.end_comments.push(comment);
    }

    /// 複数行であるかどうかを bool 型の値で返す。
    /// くくられている式が複数行であるか、かっこ式にコメントが含まれる場合は true、そうでなければ false を返す。
    pub(crate) fn is_multi_line(&self) -> bool {
        let has_trailing_comment = if let Expr::Aligned(aligned) = &self.expr {
            aligned.has_trailing_comment()
        } else {
            false
        };
        self.expr.is_multi_line()
            || !self.start_comments.is_empty()
            || !self.end_comments.is_empty()
            || has_trailing_comment
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 複数行である場合は、必ず閉じかっこのみとなる。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let current_len = acc + "(".len();
            self.expr.last_line_len_from_left(current_len) + ")".len()
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は開きかっこを描画する行のインデントの深さ
        let mut result = String::new();

        result.push('(');

        if self.is_multi_line() {
            result.push('\n');
        }

        for comment in &self.start_comments {
            result.push_str(&comment.render(depth)?);
            result.push('\n');
        }

        let formatted = self.expr.render(depth + 1)?;

        // bodyでない式は、最初の行のインデントを自分で行わない。
        // そのため、かっこのインデントの深さ + 1個分インデントを挿入する。
        if self.is_multi_line() && !self.expr.is_body() {
            result.extend(repeat_n('\t', depth + 1));
        }

        result.push_str(&formatted);

        // インデント同様に、最後の改行も行う
        if self.is_multi_line() && !self.expr.is_body() {
            result.push('\n');
        }

        for comment in &self.end_comments {
            result.push_str(&comment.render(depth)?);
            result.push('\n');
        }

        if self.is_multi_line() {
            result.extend(repeat_n('\t', depth));
        }

        result.push(')');
        Ok(result)
    }
}
