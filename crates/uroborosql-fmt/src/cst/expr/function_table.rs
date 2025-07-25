use crate::{
    cst::{Expr, Location},
    error::UroboroSQLFmtError,
};

#[derive(Debug, Clone)]
pub(crate) struct FunctionTable {
    function_expr: Box<Expr>,
    /// WITH ORDINALITY
    with_ordinality_keywords: Option<String>,
    loc: Location,
}

impl FunctionTable {
    pub(crate) fn new(
        function_expr: Expr,
        with_ordinality_keywords: Option<String>,
        loc: Location,
    ) -> Self {
        Self {
            function_expr: Box::new(function_expr),
            with_ordinality_keywords,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        let expr_len = self.function_expr.last_line_len_from_left(acc);

        // with_ordinality がある場合は、その分の長さとスペース一つ分が加算された値を返す
        if let Some(with_ordinality) = &self.with_ordinality_keywords {
            expr_len + 1 + with_ordinality.len()
        } else {
            expr_len
        }
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        self.function_expr.is_multi_line()
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.function_expr.render(depth)?);

        if let Some(with_ordinality) = &self.with_ordinality_keywords {
            result.push(' ');
            result.push_str(with_ordinality);
        }

        Ok(result)
    }
}
