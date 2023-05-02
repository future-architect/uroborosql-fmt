use itertools::repeat_n;

use crate::{
    cst::{Clause, Location, UroboroSQLFmtError},
    util::{convert_keyword_case, is_line_overflow, tab_size, to_tab_num},
};

use super::Expr;

/// FunctionCallがユーザ定義関数か組み込み関数か示すEnum
#[derive(Debug, Clone)]
pub(crate) enum FunctionCallKind {
    UserDefined,
    BuiltIn,
}

/// 関数呼び出しを表す
#[derive(Debug, Clone)]
pub(crate) struct FunctionCall {
    name: String,
    args: Vec<Expr>,
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
        args: &[Expr],
        kind: FunctionCallKind,
        loc: Location,
    ) -> FunctionCall {
        let name = name.into();
        FunctionCall {
            name,
            args: args.to_vec(),
            over_window_definition: None,
            over_keyword: "OVER".to_string(),
            _kind: kind,
            loc,
        }
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
        let arguments_last_len = if self.has_multi_line_arguments() {
            ")".len()
        } else {
            let mut current_len = acc + self.name.len() + "(".len();
            for (i, arg) in self.args.iter().enumerate() {
                current_len = arg.last_line_len_from_left(current_len);
                if i < self.args.len() - 1 {
                    // 最後以外の要素なら、"," と " " が挿入される。
                    current_len = current_len + ", ".len();
                }
            }
            current_len + ")".len()
        };

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

    /// 引数が複数行になる場合 true を返す
    fn has_multi_line_arguments(&self) -> bool {
        self.args.iter().any(|expr| expr.is_multi_line())
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

        // 現状はどのような関数名の場合でも全て予約語の大文字小文字ルールを適用する
        // 将来的には以下の機能を追加する可能性あり
        // 1. ユーザ定義関数、組み込み関数で変換ルールを変更する
        // 2. 定義ファイルに関数の大文字小文字ルールを追加する
        let func_name = convert_keyword_case(&self.name);

        result.push_str(&func_name);
        result.push('(');

        // 引数の描画を行う。
        // インデントの深さは、関数呼び出しのインデントの深さ + ",\t" として、depth + 1 を与える。
        let args = self
            .args
            .iter()
            .map(|arg| arg.render(depth + 1))
            .collect::<Result<Vec<_>, _>>()?;

        // 1行に描画した場合の文字数
        let func_char_len = format!("{}({})", func_name, args.join(", ")).len();

        // 複数行の引数がある、または、定義ファイルで設定した1行の文字列上限を超える場合、複数行で描画
        if self.has_multi_line_arguments() || is_line_overflow(func_char_len) {
            result.push('\n');

            let mut is_first = true;
            for arg in &args {
                // 関数呼び出しの深さ分インデントを挿入する
                result.extend(repeat_n('\t', depth));
                if is_first {
                    is_first = false;
                } else {
                    result.push(',');
                }
                result.push('\t');
                result.push_str(arg);
                result.push('\n');
            }
            result.extend(repeat_n('\t', depth));
        } else {
            result.push_str(&args.join(", "));
        }

        result.push(')');

        // OVER句
        if let Some(clauses) = &self.over_window_definition {
            result.push(' ');
            result.push_str(&convert_keyword_case(&self.over_keyword));
            result.push('(');

            if !clauses.is_empty() {
                result.push('\n');

                let clauses = clauses
                    .iter()
                    .map(|c| c.render(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;

                clauses.iter().for_each(|c| result.push_str(&c));

                result.extend(repeat_n('\t', depth));
            }

            result.push(')');
        }

        Ok(result)
    }
}
