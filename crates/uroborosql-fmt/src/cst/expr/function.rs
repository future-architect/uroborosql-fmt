use itertools::Itertools;

use crate::{
    cst::{add_indent, AlignInfo, AlignedExpr, Clause, Comment, Location, ParenthesizedExprList},
    error::UroboroSQLFmtError,
    util::{add_space_by_range, convert_keyword_case, is_line_overflow, tab_size, to_tab_num},
};

/// FunctionCallがユーザ定義関数か組み込み関数か示すEnum
#[derive(Debug, Clone)]
pub(crate) enum FunctionCallKind {
    UserDefined,
    BuiltIn,
}

/// 関数呼び出しの引数を表す
#[derive(Debug, Clone)]
pub(crate) struct FunctionCallArgs {
    all_distinct: Option<Clause>,
    exprs: Vec<AlignedExpr>,
    order_by: Option<Clause>,
    loc: Location,
    /// 複数行で出力するかを指定するフラグ。
    /// デフォルトでは false (つまり、単一行で出力する) になっている。
    force_multi_line: bool,
}

impl FunctionCallArgs {
    pub(crate) fn new(exprs: Vec<AlignedExpr>, loc: Location) -> FunctionCallArgs {
        Self {
            all_distinct: None,
            exprs,
            order_by: None,
            loc,
            force_multi_line: false,
        }
    }

    pub(crate) fn try_from_expr_list(
        expr_list: &crate::cst::ExprList,
        location: crate::cst::Location,
    ) -> Result<Self, crate::error::UroboroSQLFmtError> {
        let mut exprs = Vec::new();
        for item in expr_list.items() {
            if let Some(following_comment) = item.following_comments().first() {
                return Err(crate::error::UroboroSQLFmtError::Unimplemented(
                    format!(
                        "Comments following function arguments are not supported. Only trailing comments are supported.\ncomment: {}",
                        following_comment.text()
                    ),
                ));
            }

            exprs.push(item.expr().clone());
        }

        Ok(FunctionCallArgs::new(exprs, location))
    }

    pub(crate) fn force_multi_line(&self) -> bool {
        self.force_multi_line
    }

    pub(crate) fn add_expr(&mut self, cols: AlignedExpr) {
        self.loc.append(cols.loc());
        self.exprs.push(cols);
    }

    pub(crate) fn set_all_distinct(&mut self, all_distinct: Clause) {
        self.all_distinct = Some(all_distinct)
    }

    pub(crate) fn set_order_by(&mut self, order_by: Clause) {
        self.order_by = Some(order_by)
    }

    pub(crate) fn append_loc(&mut self, loc: Location) {
        self.loc.append(loc)
    }

    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // exprs は必ず1つ以上要素を持っている
        let last = self.exprs.last_mut().unwrap();
        if last.loc().is_same_line(&comment.loc()) {
            last.set_trailing_comment(comment)
        } else {
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_trailing_comment:{comment:?} is not trailing comment!"
            )))
        }
    }

    pub(crate) fn last_line_len(&self, acc: usize) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let mut current_len = acc + "(".len();

            self.exprs.iter().enumerate().for_each(|(i, col)| {
                current_len += col.last_line_len_from_left(current_len);
                if i != self.exprs.len() - 1 {
                    current_len += ", ".len()
                }
            });
            current_len + ")".len()
        }
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
            || self.all_distinct.is_some()
            || self.order_by.is_some()
            || self
                .exprs
                .iter()
                .any(|a| a.is_multi_line() || a.has_trailing_comment())
    }

    /// カラムリストをrenderする。
    /// 自身の is_multi_line() が true になる場合には複数行で描画し、false になる場合単一行で描画する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は開きかっこを描画する行のインデントの深さ
        let mut result = String::new();

        if self.is_multi_line() {
            // 各列を複数行に出力する
            result.push_str("(\n");

            // ALL/DISTINCT
            if let Some(all_distinct) = &self.all_distinct {
                result.push_str(&all_distinct.render(depth + 1)?);
            }

            // 各引数の描画
            {
                // ALL/DISTINCT、ORDER BYがある場合はインデントを1つ深くする
                let depth = if self.all_distinct.is_some() || self.order_by.is_some() {
                    depth + 1
                } else {
                    depth
                };

                // 最初の行のインデント
                add_indent(&mut result, depth + 1);

                // 各要素間の改行、カンマ、インデント
                let mut separator = "\n".to_string();
                add_indent(&mut separator, depth);
                separator.push(',');
                add_space_by_range(&mut separator, 1, tab_size());

                // Vec<AlignedExpr> -> Vec<&AlignedExpr>
                let aligned_exprs = self.exprs.iter().collect_vec();
                let align_info = AlignInfo::from(aligned_exprs);

                result.push_str(
                    &self
                        .exprs
                        .iter()
                        .map(|a| a.render_align(depth + 1, &align_info))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(&separator),
                );
            }

            // ORDER BY
            if let Some(order_by) = &self.order_by {
                result.push('\n');
                result.push_str(&order_by.render(depth + 1)?);
            } else {
                result.push('\n');
            }

            add_indent(&mut result, depth);
            result.push(')');
        } else {
            // 単一行で描画する
            // ALL/DISTINCT、ORDER BYがある場合は複数行で描画するのでこの分岐には到達しない
            result.push('(');
            result.push_str(
                &self
                    .exprs
                    .iter()
                    .map(|e| e.render(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", "),
            );
            result.push(')');
        }

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}

/// 関数呼び出しを表す
#[derive(Debug, Clone)]
pub(crate) struct FunctionCall {
    name: String,
    args: FunctionCallArgs,
    /// FILTER句が持つ where 句
    /// None ならば FILTER句自体がない
    filter_where_clause: Option<Clause>,
    filter_keyword: String,
    /// OVER句が持つ句 (PARTITION BY、ORDER BY)
    /// None であるならば OVER句自体がない
    over_window_definition: Option<Vec<Clause>>,
    over_keyword: String,
    /// ユーザ定義関数か組み込み関数かを表すフィールド
    /// 現状では使用していないが、将来的に関数呼び出しの大文字小文字ルールを変更する際に使用する可能性があるためフィールドに保持している
    _kind: FunctionCallKind,
    loc: Location,
}

impl FunctionCall {
    pub(crate) fn new(
        name: impl Into<String>,
        args: FunctionCallArgs,
        kind: FunctionCallKind,
        loc: Location,
    ) -> FunctionCall {
        let name = name.into();

        // argsが単一行で描画する設定になっている場合
        // レンダリング後の文字列の長さが定義ファイルにおける「各行の最大長」を超えないかチェックする
        let mut args = args;
        if !args.force_multi_line() {
            // 関数名と引数部分をレンダリングした際の合計文字数を計算
            let func_char_len = args.last_line_len(name.len());

            // オーバーフローしている場合はargsを複数行で描画するように変更する
            if is_line_overflow(func_char_len) {
                args.set_force_multi_line(true);
            }
        }

        FunctionCall {
            name,
            args,
            filter_where_clause: None,
            filter_keyword: convert_keyword_case("FILTER"),
            over_window_definition: None,
            over_keyword: convert_keyword_case("OVER"),
            _kind: kind,
            loc,
        }
    }

    pub(crate) fn set_filter_clause(&mut self, clause: Clause) {
        self.loc.append(clause.loc());
        self.filter_where_clause = Some(clause)
    }

    pub(crate) fn set_filter_keyword(&mut self, filter_keyword: &str) {
        self.filter_keyword = filter_keyword.to_string();
    }

    /// window_definition の句をセットする。
    pub(crate) fn set_over_window_definition(&mut self, clauses: &[Clause]) {
        let mut window_definiton = vec![];
        clauses.iter().for_each(|c| {
            self.loc.append(c.loc());
            window_definiton.push(c.clone())
        });
        self.over_window_definition = Some(window_definiton);
    }

    pub(crate) fn set_over_keyword(&mut self, over_keyword: &str) {
        self.over_keyword = over_keyword.to_string();
    }

    /// 関数呼び出しの最後の行のインデントからの文字数を返す。
    /// 引数が複数行に及ぶ場合や、OVER句の有無を考慮する。
    /// 引数 acc には、自身の左側の式の文字列の長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        let arguments_last_len = self.args.last_line_len(acc + self.name.len());

        match &self.over_window_definition {
            // OVER句があるが内容が空である場合、最後の行は "...) OVER()"
            Some(over) if over.is_empty() => {
                to_tab_num(arguments_last_len) * tab_size() + " OVER()".len()
            }
            // OVER句がある場合、最後の行は ")"
            Some(_) => ")".len(),
            None => arguments_last_len,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn append_loc(&mut self, loc: Location) {
        self.loc.append(loc)
    }

    /// 引数が複数行になる場合 true を返す
    fn has_multi_line_arguments(&self) -> bool {
        self.args.is_multi_line()
    }

    /// window定義を持つ場合 true を返す
    fn has_window_definiton_in_over(&self) -> bool {
        match &self.over_window_definition {
            Some(clauses) => !clauses.is_empty(),
            None => false,
        }
    }

    /// 関数呼び出し式が複数行になる場合 true を返す
    pub(crate) fn is_multi_line(&self) -> bool {
        self.has_window_definiton_in_over() || self.has_multi_line_arguments()
    }

    /// 関数呼び出しをフォーマットした文字列を返す。
    /// 引数が単一行に収まる場合は単一行の文字列を、複数行になる場合は引数ごとに改行を挿入した文字列を返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.name);

        // 引数の描画
        let args = self.args.render(depth)?;

        result.push_str(&args);

        // FILTER句
        if let Some(filter_clause) = &self.filter_where_clause {
            result.push(' ');
            result.push_str(&self.filter_keyword);
            result.push('(');

            result.push('\n');
            result.push_str(&filter_clause.render(depth + 1)?);

            add_indent(&mut result, depth);
            result.push(')');
        }

        // OVER句
        if let Some(clauses) = &self.over_window_definition {
            result.push(' ');
            result.push_str(&self.over_keyword);
            result.push('(');

            if !clauses.is_empty() {
                result.push('\n');

                let clauses = clauses
                    .iter()
                    .map(|c| c.render(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;

                clauses.iter().for_each(|c| result.push_str(c));

                add_indent(&mut result, depth);
            }

            result.push(')');
        }

        Ok(result)
    }
}

impl TryFrom<ParenthesizedExprList> for FunctionCallArgs {
    type Error = UroboroSQLFmtError;

    fn try_from(paren_list: ParenthesizedExprList) -> Result<Self, Self::Error> {
        if !paren_list.start_comments.is_empty() {
            return Err(UroboroSQLFmtError::Unimplemented(
                "Comments immediately after opening parenthesis in function arguments are not supported".to_string()
            ));
        }

        FunctionCallArgs::try_from_expr_list(&paren_list.expr_list, paren_list.location)
    }
}
