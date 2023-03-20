pub(crate) mod aligned;
pub(crate) mod boolean;
pub(crate) mod cond;
pub(crate) mod function;
pub(crate) mod paren;
pub(crate) mod primary;

use itertools::{repeat_n, Itertools};

use crate::util::{tab_size, to_tab_num};

use self::{
    aligned::AlignedExpr, boolean::BooleanExpr, cond::CondExpr, function::FunctionCall,
    paren::ParenExpr, primary::PrimaryExpr,
};

use super::{Comment, Location, Position, Statement, UroboroSQLFmtError};

/// 式に対応した列挙体
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    /// AS句、二項比較演算、BETWEEN述語など、縦ぞろえを行う式
    Aligned(Box<AlignedExpr>),
    /// 識別子、文字列、数値など
    Primary(Box<PrimaryExpr>),
    /// bool式
    Boolean(Box<BooleanExpr>),
    /// SELECTサブクエリ
    SelectSub(Box<SelectSubExpr>),
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
}

impl Expr {
    pub(crate) fn loc(&self) -> Location {
        match self {
            Expr::Aligned(aligned) => aligned.loc(),
            Expr::Primary(primary) => primary.loc(),
            Expr::Boolean(sep_lines) => sep_lines.loc().unwrap(),
            Expr::SelectSub(select_sub) => select_sub.loc(),
            Expr::ParenExpr(paren_expr) => paren_expr.loc(),
            Expr::Asterisk(asterisk) => asterisk.loc(),
            Expr::Cond(cond) => cond.loc(),
            Expr::Unary(unary) => unary.loc(),
            Expr::ColumnList(cols) => cols.loc(),
            Expr::FunctionCall(func_call) => func_call.loc(),
            Expr::ExprSeq(n_expr) => n_expr.loc(),
        }
    }

    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        match self {
            Expr::Aligned(aligned) => {
                // 演算子を縦ぞろえしない場合は、ここでrender()が呼ばれる
                aligned.render()
            }
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
            Expr::ParenExpr(paren_expr) => paren_expr.render(),
            Expr::Asterisk(asterisk) => asterisk.render(),
            Expr::Cond(cond) => cond.render(),
            Expr::Unary(unary) => unary.render(),
            Expr::ColumnList(cols) => cols.render(0, false),
            Expr::FunctionCall(func_call) => func_call.render(),
            Expr::ExprSeq(n_expr) => n_expr.render(),
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
            Expr::SelectSub(_) => ")".len(), // 必ずかっこ
            Expr::ParenExpr(paren) => paren.last_line_len_from_left(acc),
            Expr::Asterisk(asterisk) => asterisk.last_line_len(),
            Expr::Cond(_) => "END".len(), // "END"
            Expr::Unary(unary) => unary.last_line_len_from_left(acc),
            Expr::ColumnList(cols) => cols.last_line_len(),
            Expr::FunctionCall(func_call) => func_call.last_line_len_from_left(acc),
            Expr::Boolean(_) => unimplemented!(),
            Expr::ExprSeq(n_expr) => n_expr.last_line_len_from_left(acc),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            // aligned, primaryは上位のExpr, Bodyでset_trailing_comment()を通じてコメントを追加する
            Expr::Aligned(aligned) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for aligned\nexpr: {:?}",
                    aligned
                )));
            }
            Expr::Primary(primary) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for primary\nexpr: {:?}",
                    primary
                )));
            }

            // 下位の式にコメントを追加する
            Expr::Boolean(boolean) => {
                boolean.add_comment_to_child(comment)?;
            }
            Expr::SelectSub(select_sub) => select_sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => {
                paren_expr.add_comment_to_child(comment)?;
            }

            Expr::Cond(cond) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for conditional_expr\nexpr: {:?}",
                    cond
                )));
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
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
            // primary, aligned, boolean以外の式は現状、バインドパラメータがつくことはない
            _ => unimplemented!(),
        }
    }

    /// 複数行の式であればtrueを返す
    fn is_multi_line(&self) -> bool {
        match self {
            Expr::Boolean(_) | Expr::SelectSub(_) | Expr::Cond(_) => true,
            Expr::Primary(_) | Expr::Asterisk(_) => false,
            Expr::Aligned(aligned) => aligned.is_multi_line(),
            Expr::Unary(unary) => unary.is_multi_line(),
            Expr::ParenExpr(paren) => paren.is_multi_line(),
            Expr::FunctionCall(func_call) => func_call.is_multi_line(),
            Expr::ColumnList(_) => todo!(),
            Expr::ExprSeq(n_expr) => n_expr.is_multi_line(),
        }
    }

    // Bodyになる式(先頭のインデントと末尾の改行を行う式)であればtrue
    // そうでなければfalseを返す
    pub(crate) fn is_body(&self) -> bool {
        match self {
            Expr::Boolean(_) => true,
            Expr::Aligned(_)
            | Expr::Primary(_)
            | Expr::SelectSub(_)
            | Expr::ParenExpr(_)
            | Expr::Asterisk(_)
            | Expr::Cond(_)
            | Expr::Unary(_)
            | Expr::ColumnList(_)
            | Expr::FunctionCall(_)
            | Expr::ExprSeq(_) => false,
            // _ => unimplemented!(),
        }
    }

    /// 自身をAlignedExprでラッピングする
    pub(crate) fn to_aligned(&self) -> AlignedExpr {
        // TODO: cloneする必要があるか検討
        if let Expr::Aligned(aligned) = self {
            *aligned.clone()
        } else {
            AlignedExpr::new(self.clone(), false)
        }
    }
}

/// SELECTサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub(crate) struct SelectSubExpr {
    depth: usize,
    stmt: Statement,
    loc: Location,
}

impl SelectSubExpr {
    pub(crate) fn new(stmt: Statement, loc: Location, depth: usize) -> SelectSubExpr {
        SelectSubExpr { depth, stmt, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(&mut self, _comment: Comment) {
        unimplemented!()
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.stmt.render()?;

        result.push_str(&formatted);

        result.extend(repeat_n('\t', self.depth));
        result.push(')');

        Ok(result)
    }
}

/// アスタリスクを表す。
/// テーブル名を含む場合もある。 (例: tab.*)
#[derive(Debug, Clone)]
pub(crate) struct AsteriskExpr {
    content: String,
    loc: Location,
}

impl AsteriskExpr {
    pub(crate) fn new(content: impl Into<String>, loc: Location) -> AsteriskExpr {
        let content = content.into();
        AsteriskExpr { content, loc }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn last_line_len(&self) -> usize {
        self.content.len()
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        Ok(self.content.clone())
    }
}

/// 単項演算式
/// 例: NOT A, -B, ...
#[derive(Debug, Clone)]
pub(crate) struct UnaryExpr {
    operator: String,
    operand: Expr,
    loc: Location,
}

impl UnaryExpr {
    pub(crate) fn new(operator: impl Into<String>, operand: Expr, loc: Location) -> UnaryExpr {
        let operator = operator.into();
        UnaryExpr {
            operator,
            operand,
            loc,
        }
    }

    /// ソースコード上の位置を返す
    fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    fn last_line_len_from_left(&self, acc: usize) -> usize {
        if self.operand.is_multi_line() {
            self.operand.last_line_len()
        } else {
            // ( 演算子 '\t' 式 ) の長さ
            to_tab_num(self.operator.len() + acc) * tab_size() + self.operand.last_line_len()
        }
    }

    /// 複数行であるかどうかを返す
    fn is_multi_line(&self) -> bool {
        self.operand.is_multi_line()
    }

    /// フォーマットした文字列を返す
    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.operator);
        result.push('\t');
        result.push_str(&self.operand.render()?);

        Ok(result)
    }
}

/// 列リストを表す。
/// VALUES句、SET句で使用する
#[derive(Debug, Clone)]
pub(crate) struct ColumnList {
    cols: Vec<Expr>,
    loc: Location,
}

impl ColumnList {
    pub(crate) fn new(cols: Vec<Expr>, loc: Location) -> ColumnList {
        ColumnList { cols, loc }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn last_line_len(&self) -> usize {
        // かっこ、カンマを考慮していないため、正確な値ではない
        self.cols
            .iter()
            .fold(0, |prev, e| prev + e.last_line_tab_num())
            * tab_size()
    }

    /// カラムリストをrenderする
    /// VALUES句以外(SET句)で呼び出された場合、1行で出力する
    /// depth: インデントの深さ。SET句では0が与えられる
    /// is_one_row: VALUES句で指定される行が一つであればtrue、そうでなければfalseであるような値
    pub(crate) fn render(
        &self,
        depth: usize,
        is_one_row: bool,
    ) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        if is_one_row {
            // ValuesItemが一つだけである場合、各列を複数行に出力する

            result.push_str(" (\n");

            // 最初の行のインデント
            result.extend(repeat_n('\t', depth));

            // 各要素間の改行、カンマ、インデント
            let mut separator = "\n,".to_string();
            separator.extend(repeat_n('\t', depth));

            result.push_str(
                &self
                    .cols
                    .iter()
                    .filter_map(|e| e.render().ok())
                    .join(&separator),
            );

            result.push('\n');
            result.extend(repeat_n('\t', depth - 1));
            result.push(')');
        } else {
            // ValuesItemが複数ある場合、各行は1行に出力する

            result.extend(repeat_n('\t', depth));
            result.push('(');
            result.push_str(&self.cols.iter().filter_map(|e| e.render().ok()).join(", "));
            result.push(')');
        }

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}

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

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        Ok(self
            .exprs
            .iter()
            .map(Expr::render)
            .collect::<Result<Vec<_>, _>>()?
            .join("\t"))
    }
}

// TODO: 大文字/小文字を設定ファイルで定義できるようにする
/// 引数の文字列が識別子であれば大文字にして返す
/// 文字列リテラル、または引用符付き識別子である場合はそのままの文字列を返す
pub(crate) fn to_uppercase_identifier(elem: &str) -> String {
    if is_quoted(elem) {
        elem.to_owned()
    } else {
        elem.to_uppercase()
    }
}

/// 引数の文字列が引用符付けされているかどうかを判定する。
/// 引用符付けされている場合は true を返す。
pub(crate) fn is_quoted(elem: &str) -> bool {
    (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''))
        || (elem.starts_with('$') && elem.ends_with('$'))
}
