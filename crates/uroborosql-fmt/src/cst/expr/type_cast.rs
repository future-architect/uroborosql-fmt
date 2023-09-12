use crate::{
    cst::{Location, PrimaryExpr},
    error::UroboroSQLFmtError,
};

use super::Expr;

/// キャストを`X::type`の形式で出力する構造体
#[derive(Debug, Clone)]
pub(crate) struct TypeCast {
    expr: Expr,
    type_name: PrimaryExpr,
    loc: Location,
}

impl TypeCast {
    pub(crate) fn new(expr: Expr, type_name: PrimaryExpr, loc: Location) -> Self {
        Self {
            expr,
            type_name,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        // exprの最後の行からのインデントの長さを計算
        // 複数行判定はlast_line_len_from_left()の先で行われるのでここでは不要
        let mut len = self.expr.last_line_len_from_left(acc);

        // `::`の文字数
        len += 2;

        len = self.type_name.last_line_len_from_left(len);

        len
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        self.expr.is_multi_line()
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let formatted = self.expr.render(depth)?;

        result.push_str(&formatted);

        result.push_str("::");

        let type_formatted = self.type_name.render()?;

        result.push_str(&type_formatted);

        Ok(result)
    }
}
