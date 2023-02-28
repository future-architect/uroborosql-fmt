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

use super::{Comment, Location, Statement, UroboroSQLFmtError};

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
            Expr::FunctionCall(func_call) => func_call.render(), // _ => unimplemented!(),
        }
    }

    /// 最後の行の長さをタブ文字換算した結果を返す
    fn last_line_tab_num(&self) -> usize {
        to_tab_num(self.last_line_len())
    }

    /// 最後の行の文字列の長さを返す
    fn last_line_len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.last_line_len(),
            Expr::Aligned(aligned) => aligned.last_line_len(),
            Expr::SelectSub(_) => ")".len(), // 必ずかっこ
            Expr::ParenExpr(_) => ")".len(), // 必ずかっこ
            Expr::Asterisk(asterisk) => asterisk.last_line_len(),
            Expr::Cond(_) => "END".len(), // "END"
            Expr::Unary(unary) => unary.last_line_len(),
            Expr::ColumnList(cols) => cols.last_line_len(),
            Expr::FunctionCall(func_call) => func_call.last_line_len(),
            Expr::Boolean(_) => unimplemented!(),
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
            Expr::Boolean(_) | Expr::SelectSub(_) | Expr::ParenExpr(_) | Expr::Cond(_) => true,
            Expr::Primary(_) | Expr::Asterisk(_) => false,
            Expr::Aligned(aligned) => aligned.is_multi_line(),
            Expr::Unary(unary) => unary.is_multi_line(),
            Expr::FunctionCall(func_call) => func_call.is_multi_line(),
            Expr::ColumnList(_) => todo!(),
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
            | Expr::FunctionCall(_) => false,
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

    /// 演算子'\t'式 の最後の行の長さを返す
    fn last_line_len(&self) -> usize {
        if self.operand.is_multi_line() {
            self.operand.last_line_len()
        } else {
            to_tab_num(self.operator.len()) * tab_size() + self.operand.last_line_len()
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

// TODO: 大文字/小文字を設定ファイルで定義できるようにする
/// 引数の文字列が識別子であれば大文字にして返す
/// 文字列リテラル、または引用符付き識別子である場合はそのままの文字列を返す
pub(crate) fn to_uppercase_identifier(elem: &str) -> String {
    if (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''))
        || (elem.starts_with('$') && elem.ends_with('$'))
    {
        elem.to_owned()
    } else {
        elem.to_uppercase()
    }
}
