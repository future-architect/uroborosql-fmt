use itertools::repeat_n;

use crate::{
    cst::{Comment, Location, Statement, UroboroSQLFmtError},
    util::convert_keyword_case,
};

/// SELECTサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub(crate) struct SelectSubExpr {
    stmt: Statement,
    loc: Location,
}

impl SelectSubExpr {
    pub(crate) fn new(stmt: Statement, loc: Location) -> SelectSubExpr {
        SelectSubExpr { stmt, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(&mut self, _comment: Comment) {
        unimplemented!()
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.stmt.render(depth + 1)?;

        result.push_str(&formatted);

        result.extend(repeat_n('\t', depth));
        result.push(')');

        Ok(result)
    }
}

/// EXISTサブクエリを表す
#[derive(Debug, Clone)]
pub(crate) struct ExistsSubquery {
    exists_keyword: String,
    select_sub_expr: SelectSubExpr,
    loc: Location,
}

impl ExistsSubquery {
    pub(crate) fn new(
        exists_keyword: &str,
        select_sub_expr: SelectSubExpr,
        loc: Location,
    ) -> ExistsSubquery {
        ExistsSubquery {
            exists_keyword: exists_keyword.to_string(),
            select_sub_expr,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// EXISTSサブクエリをフォーマットした文字列を返す。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let exists_keyword = &convert_keyword_case(&self.exists_keyword);

        result.push_str(&exists_keyword);
        result += &self.select_sub_expr.render(depth)?;

        Ok(result)
    }
}
