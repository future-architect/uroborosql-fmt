use crate::{
    cst::{Comment, Location, Position},
    error::UroboroSQLFmtError,
    util::{tab_size, to_tab_num},
};

use super::Expr;

/// 複数の式をタブ文字で接続する式
/// TODO: 途中にコメントが入る場合への対応
#[derive(Debug, Clone)]
pub(crate) struct ExprSeq {
    exprs: Vec<Expr>,
    loc: Location,
}

impl ExprSeq {
    pub(crate) fn new(exprs: &[Expr]) -> ExprSeq {
        let exprs = exprs.to_vec();
        let loc = if let Some(first) = exprs.first() {
            let mut loc = first.loc();
            exprs.iter().for_each(|e| loc.append(e.loc()));
            loc
        } else {
            Location {
                start_position: Position { row: 0, col: 0 },
                end_position: Position { row: 0, col: 0 },
            }
        };
        ExprSeq { exprs, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 先頭の Expr にバインドバラメータをセットする
    pub(crate) fn set_head_comment_to_first_child(&mut self, comment: Comment) {
        if let Some(first_expr) = self.exprs.first_mut() {
            first_expr.set_head_comment(comment);
        } else {
            unimplemented!()
        }
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        self.exprs.iter().any(|e| e.is_multi_line())
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 複数行の式がある場合、最後に現れる複数行の式の長さと、それ以降の式の長さの和となる。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        let mut current_len = acc;
        for (i, e) in self.exprs.iter().enumerate() {
            if e.is_multi_line() {
                current_len = e.last_line_len()
            } else if i == 0 {
                current_len = e.last_line_len_from_left(current_len)
            } else {
                let tab_num = to_tab_num(current_len);
                current_len = e.last_line_len_from_left(tab_num * tab_size())
            }
        }
        current_len
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        Ok(self
            .exprs
            .iter()
            .map(|e| e.render(depth))
            .collect::<Result<Vec<_>, _>>()?
            .join("\t"))
    }
}
