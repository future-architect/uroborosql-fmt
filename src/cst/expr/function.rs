use itertools::repeat_n;

use crate::{
    cst::{Clause, Location, UroboroSQLFmtError},
    util::{format_keyword, tab_size, to_tab_num},
};

use super::{to_uppercase_identifier, Expr};

/// 関数呼び出しを表す
#[derive(Debug, Clone)]
pub(crate) struct FunctionCall {
    name: String,
    args: Vec<Expr>,
    /// OVER句が持つ句 (PARTITION BY、ORDER BY)
    /// None であるならば OVER句自体がない
    over_window_definition: Option<Vec<Clause>>,
    loc: Location,
    depth: usize,
}

impl FunctionCall {
    pub(crate) fn new(
        name: impl Into<String>,
        args: &[Expr],
        loc: Location,
        depth: usize,
    ) -> FunctionCall {
        let name = name.into();
        FunctionCall {
            name,
            args: args.to_vec(),
            over_window_definition: None,
            loc,
            depth,
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

    /// 関数呼び出しの最後の行の文字数を返す。
    /// 引数が複数行に及ぶ場合や、OVER句の有無を考慮する。
    pub(crate) fn last_line_len(&self) -> usize {
        let arguments_last_len = if self.has_multi_line_arguments() {
            ")".len()
        } else {
            let name_len = self.name.len();
            let args_len = self.args.len();
            let args_len: usize = self.args.iter().map(|e| e.last_line_len()).sum::<usize>()
                + ", ".len() * (args_len - 1);

            name_len + "(".len() + args_len + ")".len()
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
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let func_name = to_uppercase_identifier(&self.name);

        result.push_str(&func_name);
        result.push('(');

        // arguments
        let args = self
            .args
            .iter()
            .map(|arg| arg.render())
            .collect::<Result<Vec<_>, _>>()?;

        if self.has_multi_line_arguments() {
            result.push('\n');

            let mut is_first = true;
            for arg in &args {
                // 関数呼び出しの深さ + 1 段インデントを挿入する
                result.extend(repeat_n('\t', self.depth + 1));
                if is_first {
                    is_first = false;
                } else {
                    result.push(',');
                }
                result.push('\t');
                result.push_str(arg);
                result.push('\n');
            }
            result.extend(repeat_n('\t', self.depth + 1));
        } else {
            result.push_str(&args.join(", "));
        }

        result.push(')');

        // OVER句
        if let Some(clauses) = &self.over_window_definition {
            result.push(' ');
            result.push_str(&format_keyword("OVER"));
            result.push('(');

            if !clauses.is_empty() {
                result.push('\n');

                let clauses = clauses
                    .iter()
                    .map(Clause::render)
                    .collect::<Result<Vec<_>, _>>()?;

                clauses.iter().for_each(|c| result.push_str(&c));

                result.extend(repeat_n('\t', self.depth + 1));
            }

            result.push(')');
        }

        Ok(result)
    }
}
