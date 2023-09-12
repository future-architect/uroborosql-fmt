pub(crate) mod aligned;
pub(crate) mod asterisk;
pub(crate) mod column_list;
pub(crate) mod cond;
pub(crate) mod conflict_target;
pub(crate) mod expr_seq;
pub(crate) mod function;
pub(crate) mod paren;
pub(crate) mod primary;
pub(crate) mod subquery;
pub(crate) mod type_cast;
pub(crate) mod unary;

use crate::{error::UroboroSQLFmtError, util::to_tab_num};

use self::{
    aligned::AlignedExpr, asterisk::AsteriskExpr, cond::CondExpr, function::FunctionCall,
    paren::ParenExpr, primary::PrimaryExpr, subquery::SubExpr, type_cast::TypeCast,
    unary::UnaryExpr,
};

use super::{ColumnList, Comment, ExistsSubquery, ExprSeq, Location, SeparatedLines};

/// 式に対応した列挙型
///
/// renderの際に改行とインデントをせずに描画する（※ ただし、例外的にExpr::Booleanは先頭での改行とインデントを行う）
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    /// AS句、二項比較演算、BETWEEN述語など、縦ぞろえを行う式
    Aligned(Box<AlignedExpr>),
    /// 識別子、文字列、数値など
    Primary(Box<PrimaryExpr>),
    /// bool式、SeparatedLinesで表現
    Boolean(Box<SeparatedLines>),
    /// サブクエリ
    Sub(Box<SubExpr>),
    /// EXISTSサブクエリ
    ExistsSubquery(Box<ExistsSubquery>),
    /// かっこでくくられた式
    ParenExpr(Box<ParenExpr>),
    /// アスタリスク*
    Asterisk(Box<AsteriskExpr>),
    /// CASE式
    Cond(Box<CondExpr>),
    /// 単項演算式(NOT, +, -, ...)
    Unary(Box<UnaryExpr>),
    /// カラムリスト(VALUES句、SET句)
    ColumnList(Box<ColumnList>),
    /// 関数呼び出し
    FunctionCall(Box<FunctionCall>),
    /// N個の式の連続
    ExprSeq(Box<ExprSeq>),
    /// `::`を用いたキャスト
    TypeCast(Box<TypeCast>),
}

impl Expr {
    pub(crate) fn loc(&self) -> Location {
        match self {
            Expr::Aligned(aligned) => aligned.loc(),
            Expr::Primary(primary) => primary.loc(),
            Expr::Boolean(sep_lines) => sep_lines.loc().unwrap(),
            Expr::Sub(sub) => sub.loc(),
            Expr::ExistsSubquery(exists_sub) => exists_sub.loc(),
            Expr::ParenExpr(paren_expr) => paren_expr.loc(),
            Expr::Asterisk(asterisk) => asterisk.loc(),
            Expr::Cond(cond) => cond.loc(),
            Expr::Unary(unary) => unary.loc(),
            Expr::ColumnList(cols) => cols.loc(),
            Expr::FunctionCall(func_call) => func_call.loc(),
            Expr::ExprSeq(n_expr) => n_expr.loc(),
            Expr::TypeCast(type_cast) => type_cast.loc(),
        }
    }

    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Expr::Aligned(aligned) => {
                // 演算子を縦ぞろえしない場合は、ここでrender()が呼ばれる
                aligned.render(depth)
            }
            // Primary式、アスタリスクは改行することがないので、depthを与える必要がない。
            Expr::Primary(primary) => primary.render(),
            Expr::Asterisk(asterisk) => asterisk.render(),
            Expr::Boolean(boolean) => boolean.render(depth),
            Expr::Sub(sub) => sub.render(depth),
            Expr::ExistsSubquery(exists_sub) => exists_sub.render(depth),
            Expr::ParenExpr(paren_expr) => paren_expr.render(depth),
            Expr::Cond(cond) => cond.render(depth),
            Expr::Unary(unary) => unary.render(depth),
            Expr::ColumnList(cols) => cols.render(depth),
            Expr::FunctionCall(func_call) => func_call.render(depth),
            Expr::ExprSeq(n_expr) => n_expr.render(depth),
            Expr::TypeCast(type_cast) => type_cast.render(depth),
        }
    }

    /// 最後の行の長さをタブ文字換算した結果を返す
    fn last_line_tab_num(&self) -> usize {
        to_tab_num(self.last_line_len())
    }

    /// 自身を描画した際に、最後の行のインデントからの長さを、タブ文字換算した結果を返す。
    /// 引数 acc には、自身の左側に存在する式のインデントからの文字列の長さを与える。
    fn last_line_tab_num_from_left(&self, acc: usize) -> usize {
        to_tab_num(self.last_line_len_from_left(acc))
    }

    /// 自身がインデントの直後に描画される際の、最後の行の文字列の長さを返す
    fn last_line_len(&self) -> usize {
        self.last_line_len_from_left(0)
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    fn last_line_len_from_left(&self, acc: usize) -> usize {
        match self {
            Expr::Primary(primary) => primary.last_line_len_from_left(acc),
            Expr::Aligned(aligned) => aligned.last_line_len_from_left(acc),
            Expr::Sub(_) => ")".len(),            // 必ずかっこ
            Expr::ExistsSubquery(_) => ")".len(), // 必ずかっこ
            Expr::ParenExpr(paren) => paren.last_line_len_from_left(acc),
            Expr::Asterisk(asterisk) => asterisk.last_line_len(),
            Expr::Cond(_) => "END".len(), // "END"
            Expr::Unary(unary) => unary.last_line_len_from_left(acc),
            Expr::ColumnList(cols) => cols.last_line_len(acc),
            Expr::FunctionCall(func_call) => func_call.last_line_len_from_left(acc),
            Expr::Boolean(_) => unimplemented!(),
            Expr::ExprSeq(n_expr) => n_expr.last_line_len_from_left(acc),
            Expr::TypeCast(type_cast) => type_cast.last_line_len_from_left(acc),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            // aligned, primaryは上位のExpr, Bodyでset_trailing_comment()を通じてコメントを追加する
            Expr::Aligned(aligned) => {
                if aligned.loc().is_same_line(&comment.loc()) {
                    aligned.set_trailing_comment(comment)?;
                } else {
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "add_comment_to_child(): this comment is not trailing comment\nexpr: {:?}comment: {:?}\n",
                        aligned,
                        comment
                    )));
                }
            }
            Expr::Primary(primary) => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "add_comment_to_child(): unimplemented for primary\nexpr: {:?}",
                    primary
                )));
            }

            // 下位の式にコメントを追加する
            Expr::Boolean(boolean) => {
                boolean.add_comment_to_child(comment)?;
            }
            Expr::Sub(sub) => sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => {
                paren_expr.add_comment_to_child(comment)?;
            }

            Expr::Cond(cond) => {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "add_comment_to_child(): unimplemented for conditional_expr\nexpr: {:?}",
                    cond
                )));
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "add_comment_to_child(): unimplemented expr\nexpr: {:?}",
                    &self
                )));
            }
        }
        Ok(())
    }

    /// バインドパラメータをセットする
    /// コメントがバインドパラメータであるか(式と隣り合っているか)は呼び出し元で保証する
    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        match self {
            Expr::Primary(primary) => primary.set_head_comment(comment),
            Expr::Aligned(aligned) => aligned.set_head_comment(comment),
            Expr::Boolean(boolean) => boolean.set_head_comment(comment),
            Expr::ColumnList(col_list) => col_list.set_head_comment(comment),
            // primary, aligned, boolean以外の式は現状、バインドパラメータがつくことはない
            _ => unimplemented!(),
        }
    }

    /// 複数行の式であればtrueを返す
    fn is_multi_line(&self) -> bool {
        match self {
            Expr::Boolean(_) | Expr::Sub(_) | Expr::ExistsSubquery(_) | Expr::Cond(_) => true,
            Expr::Primary(_) | Expr::Asterisk(_) => false,
            Expr::Aligned(aligned) => aligned.is_multi_line(),
            Expr::Unary(unary) => unary.is_multi_line(),
            Expr::ParenExpr(paren) => paren.is_multi_line(),
            Expr::FunctionCall(func_call) => func_call.is_multi_line(),
            Expr::ColumnList(col_list) => col_list.is_multi_line(),
            Expr::ExprSeq(n_expr) => n_expr.is_multi_line(),
            Expr::TypeCast(type_cast) => type_cast.is_multi_line(),
        }
    }

    // Bodyになる式(先頭のインデントと末尾の改行を行う式)であればtrue
    // そうでなければfalseを返す
    pub(crate) fn is_body(&self) -> bool {
        match self {
            Expr::Boolean(_) => true,
            Expr::Aligned(_)
            | Expr::Primary(_)
            | Expr::Sub(_)
            | Expr::ExistsSubquery(_)
            | Expr::ParenExpr(_)
            | Expr::Asterisk(_)
            | Expr::Cond(_)
            | Expr::Unary(_)
            | Expr::ColumnList(_)
            | Expr::FunctionCall(_)
            | Expr::ExprSeq(_)
            | Expr::TypeCast(_) => false,
        }
    }

    /// 自身をAlignedExprでラッピングする
    pub(crate) fn to_aligned(&self) -> AlignedExpr {
        // TODO: cloneする必要があるか検討
        if let Expr::Aligned(aligned) = self {
            *aligned.clone()
        } else {
            AlignedExpr::new(self.clone())
        }
    }
}
