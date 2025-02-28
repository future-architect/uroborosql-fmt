use crate::{
    cst::Location,
    error::UroboroSQLFmtError,
    util::{add_single_space, tab_size, to_tab_num},
};

use super::Expr;

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
    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        if self.operand.is_multi_line() {
            self.operand.last_line_len()
        } else {
            // ( 演算子 '\t' 式 ) の長さ
            to_tab_num(self.operator.len() + acc) * tab_size() + self.operand.last_line_len()
        }
    }

    /// 複数行であるかどうかを返す
    pub(crate) fn is_multi_line(&self) -> bool {
        self.operand.is_multi_line()
    }

    /// フォーマットした文字列を返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.operator);
        // `NOT` のときは空白が必要
        // `@`（絶対値）のときも空白が必要（PostgreSQLでは、`@-` とすると一つのトークンとして扱われてしまうため）
        if self.operator.to_uppercase() == "NOT" || self.operator == "@" {
            add_single_space(&mut result);
        }
        result.push_str(&self.operand.render(depth)?);

        Ok(result)
    }
}
