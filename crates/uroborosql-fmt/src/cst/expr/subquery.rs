use crate::{
    cst::{add_indent, Comment, Location, Statement},
    error::UroboroSQLFmtError,
};

/// SELECTサブクエリ、DELETEサブクエリ、INSERTサブクエリ、UPDATEサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub(crate) struct SubExpr {
    stmt: Statement,
    loc: Location,
}

impl SubExpr {
    pub(crate) fn new(stmt: Statement, loc: Location) -> SubExpr {
        SubExpr { stmt, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        self.stmt.add_comment_to_child(comment)
    }

    /// 開き括弧と文の間にあるコメントを追加する
    pub(crate) fn add_start_comment(&mut self, comment: Comment) -> Result<(), UroboroSQLFmtError> {
        self.stmt.add_comment(comment);

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.stmt.render(depth + 1)?;

        result.push_str(&formatted);

        add_indent(&mut result, depth);
        result.push(')');

        Ok(result)
    }
}

/// EXISTサブクエリを表す
#[derive(Debug, Clone)]
pub(crate) struct ExistsSubquery {
    exists_keyword: String,
    select_sub_expr: SubExpr,
    loc: Location,
}

impl ExistsSubquery {
    pub(crate) fn new(
        exists_keyword: &str,
        select_sub_expr: SubExpr,
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
        let exists_keyword = &self.exists_keyword;

        result.push_str(exists_keyword);
        result += &self.select_sub_expr.render(depth)?;

        Ok(result)
    }
}

/// LATERALサブクエリを表す
#[derive(Debug, Clone)]
pub(crate) struct LateralSubquery {
    lateral_keyword: String,
    select_sub_expr: SubExpr,
    loc: Location,
}

impl LateralSubquery {
    pub(crate) fn new(
        lateral_keyword: &str,
        select_sub_expr: SubExpr,
        loc: Location,
    ) -> LateralSubquery {
        LateralSubquery {
            lateral_keyword: lateral_keyword.to_string(),
            select_sub_expr,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// LATERALサブクエリをフォーマットした文字列を返す。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let lateral_keyword = &self.lateral_keyword;

        result.push_str(lateral_keyword);
        result += &self.select_sub_expr.render(depth)?;

        Ok(result)
    }
}
