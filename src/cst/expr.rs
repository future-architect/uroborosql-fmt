pub(crate) mod aligned;
pub(crate) mod boolean;
pub(crate) mod cond;
pub(crate) mod function;
pub(crate) mod paren;
pub(crate) mod primary;
pub(crate) mod subquery;

use itertools::{repeat_n, Itertools};

use crate::util::{
    convert_identifier_case, convert_keyword_case, tab_size, to_tab_num, trim_bind_param,
};

use self::{
    aligned::AlignedExpr, boolean::BooleanExpr, cond::CondExpr, function::FunctionCall,
    paren::ParenExpr, primary::PrimaryExpr, subquery::SubExpr,
};

use super::{AlignInfo, Comment, ExistsSubquery, Location, Position, UroboroSQLFmtError};

/// 式に対応した列挙体
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    /// AS句、二項比較演算、BETWEEN述語など、縦ぞろえを行う式
    Aligned(Box<AlignedExpr>),
    /// 識別子、文字列、数値など
    Primary(Box<PrimaryExpr>),
    /// bool式
    Boolean(Box<BooleanExpr>),
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
    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.operator);
        result.push('\t');
        result.push_str(&self.operand.render(depth)?);

        Ok(result)
    }
}

/// COLLATE
#[derive(Debug, Clone)]
pub(crate) struct Collate {
    keyword: String,
    collation: String,
}

impl Collate {
    pub(crate) fn new(keyword: String, collation: String) -> Collate {
        Collate { keyword, collation }
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        result.push_str(&convert_keyword_case(&self.keyword));
        result.push('\t');

        // collationはユーザが定義することも可能であるため、識別子ルールを適用
        result.push_str(&convert_identifier_case(&self.collation));
        Ok(result)
    }
}

/// insert文のconflict_targetにおけるカラムリストの要素
#[derive(Debug, Clone)]
pub(crate) struct ConflictTargetElement {
    column: String,
    collate: Option<Collate>,
    op_class: Option<String>,
}

impl ConflictTargetElement {
    pub(crate) fn new(column: String) -> ConflictTargetElement {
        ConflictTargetElement {
            column,
            collate: None,
            op_class: None,
        }
    }

    /// COLLATEのセット
    pub(crate) fn set_collate(&mut self, collate: Collate) {
        self.collate = Some(collate);
    }

    /// op_classのセット
    pub(crate) fn set_op_class(&mut self, op_class: String) {
        self.op_class = Some(op_class);
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        result.extend(repeat_n('\t', depth));
        result.push_str(&convert_identifier_case(&self.column));

        // collationがある場合
        if let Some(collate) = &self.collate {
            result.push('\t');
            result.push_str(&collate.render()?);
        };

        // op_classがある場合
        if let Some(op_class) = &self.op_class {
            result.push('\t');
            // 演算子クラスはキーワードルールを適用
            result.push_str(&convert_keyword_case(op_class));
        };

        Ok(result)
    }
}

/// insert文のconflict_targetにおけるカラムリスト
#[derive(Debug, Clone)]
pub(crate) struct ConflictTargetColumnList {
    cols: Vec<ConflictTargetElement>,
    /// Locationを示す
    /// 現状使用していないため_locとしている
    _loc: Location,
}

impl ConflictTargetColumnList {
    pub(crate) fn new(cols: Vec<ConflictTargetElement>, loc: Location) -> ConflictTargetColumnList {
        ConflictTargetColumnList { cols, _loc: loc }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 各列を複数行に出力する
        result.push_str("(\n");

        // 最初の行のインデント
        result.extend(repeat_n('\t', depth + 1));

        // 各要素間の改行、カンマ、インデント
        let mut separator = "\n".to_string();
        separator.extend(repeat_n('\t', depth));
        separator.push_str(",\t");

        result.push_str(
            &self
                .cols
                .iter()
                .map(|a| a.render(depth - 1))
                .collect::<Result<Vec<_>, _>>()?
                .join(&separator),
        );

        result.push('\n');
        result.extend(repeat_n('\t', depth));
        result.push(')');

        Ok(result)
    }
}

/// 列のリストを表す。
#[derive(Debug, Clone)]
pub(crate) struct ColumnList {
    cols: Vec<AlignedExpr>,
    loc: Location,
    /// 複数行で出力するかを指定するフラグ。
    /// デフォルトでは false (つまり、単一行で出力する) になっている。
    force_multi_line: bool,
    /// バインドパラメータ
    head_comment: Option<String>,
}

impl ColumnList {
    pub(crate) fn new(cols: Vec<AlignedExpr>, loc: Location) -> ColumnList {
        ColumnList {
            cols,
            loc,
            force_multi_line: false,
            head_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn last_line_len(&self, acc: usize) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let mut current_len = acc + "(".len();
            if let Some(param) = &self.head_comment {
                current_len += param.len()
            };

            self.cols.iter().enumerate().for_each(|(i, col)| {
                current_len += col.last_line_len_from_left(current_len);
                if i != self.cols.len() - 1 {
                    current_len += ", ".len()
                }
            });
            current_len + ")".len()
        }
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment { text, mut loc } = comment;

        let text = trim_bind_param(text);

        self.head_comment = Some(text);
        loc.append(self.loc());
        self.loc = loc;
    }

    /// 列リストを複数行で描画するかを指定する。
    /// true を与えたら必ず複数行で描画され、false を与えたらできるだけ単一行で描画する。
    pub(crate) fn set_force_multi_line(&mut self, b: bool) {
        self.force_multi_line = b
    }

    /// 複数行で描画するかどうかを bool 型の値で取得する。
    /// 複数行で描画する場合は true を返す。
    /// 自身の is_multi_line のオプションの値だけでなく、各列が単一行かどうか、末尾コメントを持つかどうかも考慮する。
    pub(crate) fn is_multi_line(&self) -> bool {
        self.force_multi_line
            || self
                .cols
                .iter()
                .any(|a| a.is_multi_line() || a.has_trailing_comment())
    }

    /// カラムリストをrenderする。
    /// 自身の is_multi_line() が true になる場合には複数行で描画し、false になる場合単一行で描画する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は開きかっこを描画する行のインデントの深さ
        let mut result = String::new();

        // バインドパラメータがある場合、最初に描画
        if let Some(bind_param) = &self.head_comment {
            result.push_str(bind_param);
        }

        if self.is_multi_line() {
            // 各列を複数行に出力する

            result.push_str("(\n");

            // 最初の行のインデント
            result.extend(repeat_n('\t', depth + 1));

            // 各要素間の改行、カンマ、インデント
            let mut separator = "\n".to_string();
            separator.extend(repeat_n('\t', depth));
            separator.push_str(",\t");

            // Vec<AlignedExpr> -> Vec<&AlignedExpr>
            let aligned_exprs = self.cols.iter().collect_vec();
            let align_info = AlignInfo::from(aligned_exprs);

            result.push_str(
                &self
                    .cols
                    .iter()
                    .map(|a| a.render_align(depth + 1, &align_info, false))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(&separator),
            );

            result.push('\n');
            result.extend(repeat_n('\t', depth));
            result.push(')');
        } else {
            // ColumnListを単一行で描画する
            result.push('(');
            result.push_str(
                &self
                    .cols
                    .iter()
                    .filter_map(|e| e.render(depth + 1).ok())
                    .join(", "),
            );
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

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        Ok(self
            .exprs
            .iter()
            .map(|e| e.render(depth))
            .collect::<Result<Vec<_>, _>>()?
            .join("\t"))
    }
}
