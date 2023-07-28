pub(crate) mod insert;
pub(crate) mod separeted_lines;
pub(crate) mod single_line;
pub(crate) mod with;

use crate::error::UroboroSQLFmtError;

use self::{
    insert::InsertBody, separeted_lines::SeparatedLines, single_line::SingleLine, with::WithBody,
};

use super::{BooleanExpr, Comment, Expr, Location};

/// 句の本体を表す
#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    Insert(Box<InsertBody>),
    With(Box<WithBody>),
    /// Clause と Expr を単一行で描画する際の Body
    SingleLine(Box<SingleLine>),
}

impl Body {
    /// 本体の要素が空である場合 None を返す
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
            Body::Insert(insert) => Some(insert.loc()),
            Body::With(with) => with.loc(),
            Body::SingleLine(expr_body) => Some(expr_body.loc()),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(depth),
            Body::BooleanExpr(bool_expr) => bool_expr.render(depth),
            Body::Insert(insert) => insert.render(depth),
            Body::With(with) => with.render(depth),
            Body::SingleLine(single_line) => single_line.render(depth),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment)?,
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment)?,
            Body::Insert(insert) => insert.add_comment_to_child(comment)?,
            Body::With(with) => with.add_comment_to_child(comment)?,
            Body::SingleLine(single_line) => single_line.add_comment_to_child(comment)?,
        }

        Ok(())
    }

    /// bodyの要素が空であるかどうかを返す
    pub(crate) fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::BooleanExpr(bool_expr) => bool_expr.is_empty(),
            Body::With(_) => false, // WithBodyには必ずwith_contentsが含まれる
            Body::Insert(_) => false, // InsertBodyには必ずtable_nameが含まれる
            Body::SingleLine(_) => false,
        }
    }

    /// 一つのExprからなるBodyを生成し返す
    pub(crate) fn with_expr(expr: Expr) -> Body {
        if expr.is_body() {
            // Bodyである場合はそのまま返せばよい
            if let Expr::Boolean(boolean) = expr {
                Body::BooleanExpr(*boolean)
            } else {
                // error
                unimplemented!()
            }
        } else {
            // Bodyでない場合、SeparatedLinesにして返す
            let mut sep_lines = SeparatedLines::new("");
            sep_lines.add_expr(expr.to_aligned());
            Body::SepLines(sep_lines)
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
            Body::BooleanExpr(boolean) => boolean.try_set_head_comment(comment),
            Body::Insert(_) => false,
            Body::With(_) => false,
            Body::SingleLine(single_line) => single_line.try_set_head_comment(comment),
        }
    }
}
