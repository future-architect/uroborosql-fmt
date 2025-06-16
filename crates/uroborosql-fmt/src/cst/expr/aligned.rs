use crate::{
    cst::{add_indent, Comment, Location},
    error::UroboroSQLFmtError,
    util::{add_single_space, add_space_by_range, tab_size, to_tab_num},
};

use super::Expr;

/// AlignedExprの演算子、コメントを縦ぞろえする際に使用する情報を含む構造体
#[derive(Debug)]
pub(crate) struct AlignInfo {
    /// 演算子を持つAlignedExprが含まれていればtrue
    has_op: bool,

    /// 演算子の最長の長さをタブ換算したもの
    ///
    /// AlignedExprの式が1つも演算子を持っていない場合はNone
    max_op_tab_num: Option<usize>,

    /// 演算子までの最長の長さをタブ換算したもの
    ///
    /// AlignedExprの式が1つも演算子を持っていない場合はNone
    max_tab_num_to_op: Option<usize>,

    /// 行末コメントまでの最長の長さをタブ換算したもの
    ///
    /// AlignedExprの式が1つも行末コメントを持っていない場合はNone
    max_tab_num_to_comment: Option<usize>,
}

impl From<Vec<&AlignedExpr>> for AlignInfo {
    /// AlignedExprのVecからAlignInfoを生成する
    fn from(aligned_exprs: Vec<&AlignedExpr>) -> Self {
        let has_op = aligned_exprs.iter().any(|aligned| aligned.has_rhs());

        let has_comment = aligned_exprs.iter().any(|aligned| {
            aligned.trailing_comment.is_some() || aligned.lhs_trailing_comment.is_some()
        });

        // 演算子の最長の長さをタブ換算したもの
        let max_op_tab_num = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.op_tab_num().unwrap_or(0))
                .max()
        } else {
            None
        };

        // 演算子までの最長の長さをタブ換算したもの
        let max_tab_num_to_op = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.lhs_tab_num())
                .max()
        } else {
            None
        };

        // 行末コメントまでの最長の長さをタブ換算したもの
        let max_tab_num_to_comment = if has_comment {
            aligned_exprs
                .iter()
                .flat_map(|aligned| aligned.tab_num_to_comment(max_tab_num_to_op))
                .max()
        } else {
            None
        };

        AlignInfo {
            has_op,
            max_op_tab_num,
            max_tab_num_to_op,
            max_tab_num_to_comment,
        }
    }
}

impl AlignInfo {
    /// 演算子を持つAlignedExprが含まれているかどうか
    pub(crate) fn has_op(&self) -> bool {
        self.has_op
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
}

impl AlignedExpr {
    pub(crate) fn new(lhs: Expr) -> AlignedExpr {
        let loc = lhs.loc();
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            trailing_comment: None,
            lhs_trailing_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// opのタブ文字換算の長さを返す (opが存在しない場合はNone)
    ///
    /// 例えばtab_sizeが4、opがbetweenの場合
    ///
    /// op_tab_num() => 2
    fn op_tab_num(&self) -> Option<usize> {
        self.op.as_ref().map(|op| to_tab_num(op.len()))
    }

    /// 最後の行のインデントからの文字列の長さを返す。
    /// 引数 acc には、自身の左側の式についてインデントからの文字列の長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        match (&self.op, &self.rhs) {
            // 右辺があり、複数行ではない場合、(左辺'\t'演算子'\t'右辺) の長さを返す
            (Some(_), Some(rhs)) if !rhs.is_multi_line() => {
                (self.lhs.last_line_tab_num_from_left(acc) + self.op_tab_num().unwrap())
                    * tab_size()
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
        if comment.is_block_comment() {
            // 複数行コメント
            return Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_trailing_comment:{comment:?} is not trailing comment!"
            )));
        }

        let Comment { text, loc } = comment;
        // 1. 初めのハイフンを削除
        // 2. 空白、スペースなどを削除
        // 3. "--" を付与
        let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

        self.trailing_comment = Some(trailing_comment);
        self.loc.append(loc);

        Ok(())
    }

    /// 左辺のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_lhs_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_lhs_trailing_comment:{comment:?} is not trailing comment!"
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

    /// 演算子と右辺の式を追加する
    pub(crate) fn add_rhs(&mut self, op: Option<String>, rhs: Expr) {
        self.loc.append(rhs.loc());
        self.op = op;
        self.rhs = Some(rhs);
    }

    /// 右辺があるかどうかをboolで返す
    pub(crate) fn has_rhs(&self) -> bool {
        self.rhs.is_some()
    }

    /// 複数行であるかどうかを返す
    pub(crate) fn is_multi_line(&self) -> bool {
        self.lhs.is_multi_line() || self.rhs.as_ref().map(Expr::is_multi_line).unwrap_or(false)
    }

    // 演算子までの長さをタブ単位で返す
    // 左辺の長さを返せばよい
    pub(crate) fn lhs_tab_num(&self) -> usize {
        if self.lhs_trailing_comment.is_some() {
            // trailing commentが左辺にある場合、改行するため0
            0
        } else {
            self.lhs.last_line_tab_num()
        }
    }

    // 演算子までの長さを返す
    // 左辺の長さを返せばよい
    pub(crate) fn lhs_last_line_len(&self) -> usize {
        if self.lhs_trailing_comment.is_some() {
            // trailing commentが左辺にある場合、改行するため0
            0
        } else {
            self.lhs.last_line_len()
        }
    }

    /// 末尾コメントを持っている場合 true を返す。
    pub(crate) fn has_trailing_comment(&self) -> bool {
        self.trailing_comment.is_some() || self.lhs_trailing_comment.is_some()
    }

    // 演算子から末尾コメントまでの長さを返す
    pub(crate) fn tab_num_to_comment(&self, max_tab_num_to_op: Option<usize>) -> Option<usize> {
        match (max_tab_num_to_op, &self.rhs) {
            // コメント以外にそろえる対象があり、この式が右辺を持つ場合は右辺の長さ
            (Some(_), Some(rhs)) => Some(rhs.last_line_tab_num()),
            // コメント以外に揃える対象があり、右辺を補完しない場合、0
            (Some(_), None) => Some(0),
            // そろえる対象がコメントだけであるとき、左辺の長さ
            (_, _) => Some(self.lhs.last_line_tab_num()),
        }
    }

    /// 演算子・コメントの縦ぞろえをせずにrenderする
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // 自身のみからAlignInfo作成
        let align_info = &AlignInfo::from(vec![self]);

        self.render_align(depth, align_info)
    }

    /// 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub(crate) fn render_align(
        &self,
        depth: usize,
        // 縦揃え対象AligendExpr(自分を含む)の情報
        align_info: &AlignInfo,
    ) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 演算子の最長の長さをタブ換算したもの
        // AlignedExprの式が1つも演算子を持っていない場合はNone
        let max_op_tab_num = align_info.max_op_tab_num;

        // 演算子までの最長の長さをタブ換算したもの
        // AlignedExprの式が1つも演算子を持っていない場合はNone
        let max_tab_num_to_op = align_info.max_tab_num_to_op;

        // 行末コメントまでの最長の長さをタブ換算したもの
        // AlignedExprの式が1つも行末コメントを持っていない場合はNone
        let max_tab_num_to_comment = align_info.max_tab_num_to_comment;

        // 左辺をrender
        let formatted = self.lhs.render(depth)?;
        result.push_str(&formatted);

        // 演算子を持つAligendExprが存在するかどうか (=演算子で縦揃えをするかどうか)
        let any_expr_has_op = align_info.has_op();

        if any_expr_has_op {
            // いずれかのAligendExprが演算子を持つので必ず共にSome(_)
            let max_op_tab_num = max_op_tab_num.unwrap();
            let max_tab_num_to_op = max_tab_num_to_op.unwrap();

            // 自身が演算子を持つ場合、演算子、右辺を縦揃えする
            if let Some(op) = &self.op {
                // 左辺に行末コメントがある場合
                // (現状binary_expressionの左辺に行末コメントがある場合はCSTの構成の時点で未対応)
                if let Some(comment_str) = &self.lhs_trailing_comment {
                    if depth < 1 {
                        // 左辺に行末コメントがある場合、右辺の直前にタブ文字が挿入されるため、
                        // インデントの深さ(depth)は1以上でなければならない。
                        return Err(UroboroSQLFmtError::Rendering(
                            "AlignedExpr::render_align(): The depth must be bigger than 0"
                                .to_owned(),
                        ));
                    }

                    add_single_space(&mut result);
                    result.push_str(comment_str);
                    result.push('\n');

                    // インデントを挿入
                    add_indent(&mut result, depth - 1);
                }

                // 左辺がCASE文の場合はopの前に改行してdepthだけタブを挿入
                if matches!(self.lhs, Expr::Cond(_)) {
                    result.push('\n');
                    add_indent(&mut result, depth);
                }

                // 縦揃え対象opの直前までタブを挿入
                //
                // 全ての左辺に trailing comment があり、以下の3条件を満たす場合でも、スペースによるインデント対応前と同じ挙動にするために
                // `1.max(...)` として最低でも一つのインデントが行われるようにしている
                //
                // 1. self.lhs_last_line_len() == 0
                // 2. self.lhs_tab_num() == 0
                // 3. max_tab_num_to_op == 0
                add_space_by_range(
                    &mut result,
                    self.lhs_last_line_len(),
                    1.max(self.lhs_tab_num()) * tab_size(),
                );
                add_indent(&mut result, max_tab_num_to_op - self.lhs_tab_num());

                // from句以外はopを挿入
                if !op.is_empty() {
                    result.push_str(op);

                    // 右辺が存在してCASE文ではない場合はタブを挿入
                    // CASE文の場合はopの直後で改行するため、opの後にはタブを挿入しない
                    if self.rhs.is_some() && !matches!(&self.rhs, Some(Expr::Cond(_))) {
                        add_space_by_range(&mut result, op.len(), max_op_tab_num * tab_size());
                    }
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = if matches!(rhs, Expr::Cond(_)) {
                        // 右辺がCASE文の場合は改行してタブを挿入
                        result.push('\n');
                        add_indent(&mut result, depth + 1);
                        // 1つ深いところでrender
                        rhs.render(depth + 1)?
                    } else {
                        rhs.render(depth)?
                    };
                    result.push_str(&formatted);
                }
            // 演算子を持たないが右辺が存在する場合、右辺を他の右辺に縦揃えする
            } else if self.rhs.is_some() {
                if let Some(comment_str) = &self.lhs_trailing_comment {
                    if depth < 1 {
                        // 左辺に行末コメントがある場合、右辺の直前にタブ文字が挿入されるため、
                        // インデントの深さ(depth)は1以上でなければならない。
                        return Err(UroboroSQLFmtError::Rendering(
                            "AlignedExpr::render_align(): The depth must be bigger than 0"
                                .to_owned(),
                        ));
                    }

                    add_single_space(&mut result);
                    result.push_str(comment_str);
                    result.push('\n');

                    // インデントを挿入
                    add_indent(&mut result, depth - 1);
                }

                // 左辺がCASE文の場合はopの前に改行してdepthだけタブを挿入
                if matches!(self.lhs, Expr::Cond(_)) {
                    result.push('\n');
                    add_indent(&mut result, depth);
                }

                // 縦揃え対象opの直前までタブを挿入
                //
                // 全ての左辺に trailing comment があり、以下の3条件を満たす場合でも、スペースによるインデント対応前と同じ挙動にするために
                // `1.max(...)` として最低でも一つのインデントが行われるようにしている
                //
                // 1. self.lhs_last_line_len() == 0
                // 2. self.lhs_tab_num() == 0
                // 3. max_tab_num_to_op == 0
                add_space_by_range(
                    &mut result,
                    self.lhs_last_line_len(),
                    1.max(self.lhs_tab_num()) * tab_size(),
                );
                add_indent(&mut result, max_tab_num_to_op - self.lhs_tab_num());

                // 右辺が存在してCASE文ではない場合はタブを挿入
                // CASE文の場合はopの直後で改行するため、opの後にはタブを挿入しない
                if self.rhs.is_some() && !matches!(&self.rhs, Some(Expr::Cond(_))) {
                    let tab_num = max_op_tab_num; // self.op != Noneならop_tab_num != None
                    add_indent(&mut result, tab_num);
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = if matches!(rhs, Expr::Cond(_)) {
                        // 右辺がCASE文の場合は改行してタブを挿入
                        result.push('\n');
                        add_indent(&mut result, depth + 1);
                        // 1つ深いところでrender
                        rhs.render(depth + 1)?
                    } else {
                        rhs.render(depth)?
                    };
                    result.push_str(&formatted);
                }
            }
        }

        // 行末コメントが存在する場合
        if let Some(trailing_comment) = &self.trailing_comment {
            // 行末コメントが存在する場合はmax_tab_num_to_commentはSome(_)
            let max_tab_num_to_comment = max_tab_num_to_comment.unwrap();

            if any_expr_has_op {
                // いずれかのAligendExprが演算子を持つので必ず共にSome(_)
                let max_op_tab_num = max_op_tab_num.unwrap();
                let max_tab_num_to_op = max_tab_num_to_op.unwrap();

                let (start_col, end_col) = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // trailing_commentがある場合、max_tab_num_to_commentは必ずSome(_)
                    let start_col = rhs.last_line_len();
                    let end_col = max_tab_num_to_comment * tab_size()
                        + if rhs.is_multi_line() {
                            // 右辺が複数行である場合、最後の行に左辺と演算子はないため、その分タブで埋める
                            (max_tab_num_to_op + max_op_tab_num) * tab_size()
                        } else {
                            0
                        };

                    (start_col, end_col)
                } else {
                    // 右辺がない場合は
                    // コメントまでの最長 + 演算子の長さ + 左辺の最大長からの差分
                    let start_col = self.lhs.last_line_len();
                    let end_col =
                        (max_tab_num_to_comment + max_op_tab_num + max_tab_num_to_op) * tab_size();
                    (start_col, end_col)
                };

                add_space_by_range(&mut result, start_col, end_col);
                result.push_str(trailing_comment);
            } else {
                // 全てのAligendExprが演算子を持たない場合
                // 左辺だけを考慮すれば良い
                add_space_by_range(
                    &mut result,
                    self.lhs.last_line_len(),
                    max_tab_num_to_comment * tab_size(),
                );
                result.push_str(trailing_comment);
            }
        }

        Ok(result)
    }

    /// 左辺がCASE文であればtrueを返す
    pub(crate) fn is_lhs_cond(&self) -> bool {
        matches!(&self.lhs, Expr::Cond(_))
    }
}
