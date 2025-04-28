pub(crate) mod insert;
pub(crate) mod select;
pub(crate) mod separeted_lines;
pub(crate) mod single_line;
pub(crate) mod values;
pub(crate) mod with;

use values::ValuesBody;

use crate::error::UroboroSQLFmtError;

use self::{
    insert::InsertBody, select::SelectBody, separeted_lines::SeparatedLines,
    single_line::SingleLine, with::WithBody,
};

use super::{Comment, Expr, Location};

/// 句の本体を表す列挙型
///
/// renderの際に改行とインデントをしてから描画する（※ ただし、例外的にBody::SingleLineは先頭での改行とインデントを行わない）
#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    Insert(Box<InsertBody>),
    Select(Box<SelectBody>),
    With(Box<WithBody>),
    /// Clause と Expr を単一行で描画する際の Body
    SingleLine(Box<SingleLine>),
    Values(Box<ValuesBody>),
}

impl From<Expr> for Body {
    /// 一つのExprからなるBodyを生成し返す
    fn from(expr: Expr) -> Body {
        if expr.is_body() {
            // BooleanはSeparatedLinesで表現されるので、そのSeparatedLinesをBodyとして返す
            if let Expr::Boolean(boolean) = expr {
                Body::SepLines(*boolean)
            } else {
                // 現状Expr::Boolean()以外にBodyとなりうるExprは存在しないので到達しない
                unreachable!()
            }
        } else {
            // Bodyでない場合、SeparatedLinesにして返す
            let mut sep_lines = SeparatedLines::new();
            sep_lines.add_expr(expr.to_aligned(), None, vec![]);
            Body::SepLines(sep_lines)
        }
    }
}

impl Body {
    /// 本体の要素が空である場合 None を返す
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::Insert(insert) => Some(insert.loc()),
            Body::With(with) => with.loc(),
            Body::SingleLine(expr_body) => Some(expr_body.loc()),
            Body::Select(select) => select.loc(),
            Body::Values(values) => Some(values.loc()),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(depth),
            Body::Insert(insert) => insert.render(depth),
            Body::With(with) => with.render(depth),
            Body::SingleLine(single_line) => single_line.render(depth),
            Body::Select(select) => select.render(depth),
            Body::Values(values) => values.render(depth),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment)?,
            Body::Insert(insert) => insert.add_comment_to_child(comment)?,
            Body::With(with) => with.add_comment_to_child(comment)?,
            Body::SingleLine(single_line) => single_line.add_comment_to_child(comment)?,
            Body::Select(select) => select.add_comment_to_child(comment)?,
            Body::Values(values) => values.add_comment_to_child(comment)?,
        }

        Ok(())
    }

    /// bodyの要素が空であるかどうかを返す
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::With(_) => false, // WithBodyには必ずwith_contentsが含まれる
            Body::Insert(_) => false, // InsertBodyには必ずtable_nameが含まれる
            Body::SingleLine(_) => false,
            Body::Select(select) => select.is_empty(),
            Body::Values(_) => false, // ValuesBodyには必ずrowが含まれる
        }
    }

    /// 単一行の Clause の Body となる SingleLineを生成する
    pub(crate) fn to_single_line(expr: Expr) -> Body {
        Body::SingleLine(Box::new(SingleLine::new(expr)))
    }

    /// Body に含まれる最初の式にバインドパラメータをセットすることを試みる。
    /// セットできた場合は true を返し、できなかった場合は false を返す。
    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.try_set_head_comment(comment),
            Body::Insert(_) => false,
            Body::With(_) => false,
            Body::SingleLine(single_line) => single_line.try_set_head_comment(comment),
            Body::Select(select) => select.try_set_head_comment(comment),
            Body::Values(values) => values.try_set_head_comment(comment),
        }
    }
}
