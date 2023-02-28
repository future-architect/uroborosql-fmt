use itertools::repeat_n;

use crate::{
    config::CONFIG,
    cst::{Comment, Location, UroboroSQLFmtError},
    util::{format_keyword, tab_size, to_tab_num},
};

use super::{primary::PrimaryExpr, Expr};

/// AlignedExprの演算子、コメントを縦ぞろえする際に使用する情報を含む構造体
#[derive(Debug)]
pub(crate) struct AlignInfo {
    /// 演算子自身の最長の長さ
    max_op_tab_num: Option<usize>,
    /// 演算子までの最長の長さ
    max_tab_num_to_op: Option<usize>,
    /// 行末コメントまでの最長の長さ
    max_tab_num_to_comment: Option<usize>,
}

impl From<Vec<&AlignedExpr>> for AlignInfo {
    /// AlignedExprのVecからAlignInfoを生成する
    fn from(aligned_exprs: Vec<&AlignedExpr>) -> Self {
        let has_op = aligned_exprs.iter().any(|aligned| aligned.has_rhs());

        let has_comment = aligned_exprs.iter().any(|aligned| {
            aligned.trailing_comment.is_some() || aligned.lhs_trailing_comment.is_some()
        });

        // 演算子自体の長さ
        let max_op_tab_num = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.op_tab_num().unwrap_or(0))
                .max()
        } else {
            None
        };

        let max_tab_num_to_op = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.lhs_tab_num())
                .max()
        } else {
            None
        };

        let max_tab_num_to_comment = if has_comment {
            aligned_exprs
                .iter()
                .flat_map(|aligned| aligned.tab_num_to_comment(max_tab_num_to_op))
                .max()
        } else {
            None
        };

        AlignInfo {
            max_op_tab_num,
            max_tab_num_to_op,
            max_tab_num_to_comment,
        }
    }
}

impl AlignInfo {
    pub(crate) fn new(
        max_op_tab_num: Option<usize>,
        max_tab_num_to_op: Option<usize>,
        max_tab_num_to_comment: Option<usize>,
    ) -> AlignInfo {
        AlignInfo {
            max_op_tab_num,
            max_tab_num_to_op,
            max_tab_num_to_comment,
        }
    }
}

/// Bodyの要素となる、縦ぞろえの対象(演算子、AS、末尾コメント)を持つ式を表す
#[derive(Debug, Clone)]
pub(crate) struct AlignedExpr {
    lhs: Expr,
    rhs: Option<Expr>,
    op: Option<String>,
    loc: Location,
    /// 行末コメント
    trailing_comment: Option<String>,
    /// 左辺の直後に現れる行末コメント
    lhs_trailing_comment: Option<String>,
    /// エイリアス式であるかどうか
    is_alias: bool,
}

impl AlignedExpr {
    pub(crate) fn new(lhs: Expr, is_alias: bool) -> AlignedExpr {
        let loc = lhs.loc();
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            trailing_comment: None,
            lhs_trailing_comment: None,
            is_alias,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// opのタブ文字換算の長さを返す
    fn op_tab_num(&self) -> Option<usize> {
        self.op.as_ref().map(|op| to_tab_num(op.len()))
    }

    /// 最後の行の文字列の長さを返す
    pub(crate) fn last_line_len(&self) -> usize {
        match (&self.op, &self.rhs) {
            // 右辺があり、複数行ではない場合、(左辺'\t'演算子'\t'右辺) の長さを返す
            (Some(_), Some(rhs)) if !rhs.is_multi_line() => {
                (self.lhs.last_line_tab_num() + self.op_tab_num().unwrap()) * tab_size()
                    + rhs.last_line_len()
            }
            // 右辺があり、複数行である場合、右辺の長さを返す
            (Some(_), Some(rhs)) => rhs.last_line_len(),
            _ => self.lhs.last_line_len(),
        }
    }

    /// 右辺(行全体)のtrailing_commentをセットする
    /// 複数行コメントを与えた場合エラーを返す
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperationError(format!(
                "set_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            let Comment { text, loc } = comment;
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// 左辺のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_lhs_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperationError(format!(
                "set_lhs_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            // 行コメント
            let Comment { text, loc } = comment;
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.lhs_trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// 左辺にバインドパラメータをセットする
    /// 隣り合っているかどうかは呼び出しもとでチェック済み
    pub fn set_head_comment(&mut self, comment: Comment) {
        self.lhs.set_head_comment(comment);
    }

    // 演算子と右辺の式を追加する
    pub(crate) fn add_rhs(&mut self, op: impl Into<String>, rhs: Expr) {
        self.loc.append(rhs.loc());
        self.op = Some(op.into());
        self.rhs = Some(rhs);
    }

    // 右辺があるかどうかをboolで返す
    pub(crate) fn has_rhs(&self) -> bool {
        self.rhs.is_some()
    }

    /// 複数行であるかどうかを返す
    pub(crate) fn is_multi_line(&self) -> bool {
        self.lhs.is_multi_line() || self.rhs.as_ref().map(Expr::is_multi_line).unwrap_or(false)
    }

    // 演算子までの長さを返す
    // 左辺の長さを返せばよい
    pub(crate) fn lhs_tab_num(&self) -> usize {
        if self.lhs_trailing_comment.is_some() {
            // trailing commentが左辺にある場合、改行するため0
            0
        } else {
            self.lhs.last_line_tab_num()
        }
    }

    // 演算子から末尾コメントまでの長さを返す
    pub(crate) fn tab_num_to_comment(&self, max_tab_num_to_op: Option<usize>) -> Option<usize> {
        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        match (max_tab_num_to_op, &self.rhs) {
            // コメント以外にそろえる対象があり、この式が右辺を持つ場合は右辺の長さ
            (Some(_), Some(rhs)) => Some(rhs.last_line_tab_num()),
            // コメント以外に揃える対象があり、右辺を左辺で補完する場合、左辺の長さ
            (Some(_), None)
                if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk =>
            {
                if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = *strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right, primary.loc());
                    Some(new_prim.last_line_tab_num())
                } else {
                    Some(self.lhs.last_line_tab_num())
                }
            }
            // コメント以外に揃える対象があり、右辺を左辺を保管しない場合、0
            (Some(_), None) => Some(0),
            // そろえる対象がコメントだけであるとき、左辺の長さ
            _ => Some(self.lhs.last_line_tab_num()),
        }
    }

    /// 演算子・コメントの縦ぞろえをせずにrenderする
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let tab_num_to_op = if self.has_rhs() {
            Some(self.lhs_tab_num())
        } else {
            None
        };
        self.render_align(
            0,
            &AlignInfo::new(
                self.op_tab_num(),
                tab_num_to_op,
                self.tab_num_to_comment(tab_num_to_op),
            ),
            false,
        )
    }

    /// 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub(crate) fn render_align(
        &self,
        depth: usize,
        align_info: &AlignInfo,
        is_from_body: bool,
    ) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let max_op_tab_num = align_info.max_op_tab_num;
        let max_tab_num_to_op = align_info.max_tab_num_to_op;
        let max_tab_num_to_comment = align_info.max_tab_num_to_comment;

        //左辺をrender
        let formatted = self.lhs.render()?;
        result.push_str(&formatted);

        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        // 演算子と右辺をrender
        match (&self.op, max_op_tab_num, max_tab_num_to_op) {
            (Some(op), Some(max_op_tab_num), Some(max_tab_num)) => {
                if let Some(comment_str) = &self.lhs_trailing_comment {
                    result.push('\t');
                    result.push_str(comment_str);
                    result.push('\n');

                    // インデントを挿入
                    result.extend(repeat_n('\t', depth));
                }

                let tab_num = max_tab_num - self.lhs_tab_num();
                result.extend(repeat_n('\t', tab_num));

                result.push('\t');

                // from句以外はopを挿入
                if !is_from_body {
                    result.push_str(op);
                    let tab_num = max_op_tab_num - self.op_tab_num().unwrap(); // self.op != Noneならop_tab_num != None
                    result.extend(repeat_n('\t', tab_num + 1));
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = rhs.render()?;
                    result.push_str(&formatted);
                }
            }
            // AS補完する場合
            (None, _, Some(max_tab_num))
                if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk =>
            {
                let tab_num = max_tab_num - self.lhs.last_line_tab_num();
                result.extend(repeat_n('\t', tab_num));

                if !is_from_body {
                    result.push('\t');
                    result.push_str(&format_keyword("AS"));
                }
                // エイリアス補完はすべての演算子が"AS"であるかないため、すべての演算子の長さ(op_tab_num())は等しい
                result.push('\t');

                let formatted = if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = *strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right, primary.loc());
                    new_prim.render().unwrap()
                } else {
                    self.lhs.render().unwrap()
                };

                result.push_str(&formatted);
            }
            (_, _, _) => (),
        }

        // 末尾コメントをrender
        match (&self.trailing_comment, max_op_tab_num, max_tab_num_to_op) {
            // 末尾コメントが存在し、ほかのそろえる対象が存在する場合
            (Some(comment), Some(max_op_tab_num), Some(max_tab_num)) => {
                let tab_num = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // trailing_commentがある場合、max_tab_num_to_commentは必ずSome(_)
                    max_tab_num_to_comment.unwrap() - rhs.last_line_tab_num()
                        + if rhs.is_multi_line() {
                            // 右辺が複数行である場合、最後の行に左辺と演算子はないため、その分タブで埋める
                            max_tab_num + max_op_tab_num
                        } else {
                            0
                        }
                } else if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk {
                    let lhs_tab_num = if let Expr::Primary(primary) = &self.lhs {
                        let str = primary.elements().first().unwrap();
                        let strs: Vec<&str> = str.split('.').collect();
                        let right = *strs.last().unwrap();
                        let new_prim = PrimaryExpr::new(right, primary.loc());
                        new_prim.last_line_tab_num()
                    } else {
                        self.lhs.last_line_tab_num()
                    };
                    // AS補完する場合には、右辺に左辺と同じ式を挿入する
                    max_tab_num_to_comment.unwrap() - lhs_tab_num
                } else {
                    // 右辺がない場合は
                    // コメントまでの最長 + 演算子の長さ + 左辺の最大長からの差分
                    max_tab_num_to_comment.unwrap()
                        + (if is_from_body { 0 } else { max_op_tab_num })
                        + max_tab_num
                        - self.lhs.last_line_tab_num()
                };

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            // 末尾コメントが存在し、ほかにはそろえる対象が存在しない場合
            (Some(comment), _, None) => {
                // max_tab_num_to_opがNoneであればそろえる対象はない
                let tab_num = max_tab_num_to_comment.unwrap() - self.lhs.last_line_tab_num();

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            _ => (),
        }

        Ok(result)
    }
}
