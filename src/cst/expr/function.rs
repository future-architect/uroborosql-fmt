use itertools::repeat_n;

use crate::cst::{Location, UroboroSQLFmtError};

use super::{to_uppercase_identifier, Expr};

/// 関数呼び出しを表す
#[derive(Debug, Clone)]
pub(crate) struct FunctionCall {
    name: String,
    args: Vec<Expr>,
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
            loc,
            depth,
        }
    }

    /// 関数名'('引数')' の長さを返す
    /// 引数が複数行になる場合、')'の長さになる
    pub(crate) fn last_line_len(&self) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let name_len = self.name.len();
            let args_len = self.args.len();
            let args_len: usize = self.args.iter().map(|e| e.last_line_len()).sum::<usize>()
                + ", ".len() * (args_len - 1);

            name_len + "(".len() + args_len + ")".len()
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        self.args.iter().any(|expr| expr.is_multi_line())
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

        if self.is_multi_line() {
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

        Ok(result)
    }
}
