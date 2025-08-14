pub(crate) mod element_list;
pub(crate) mod name_list;

use crate::{
    cst::{ColumnList, Location},
    error::UroboroSQLFmtError,
};

/// テーブル関数のエイリアス句を表す。
/// 例: t(i, v), (i int, v text), t(i int, v text) など
#[derive(Debug, Clone)]
pub(crate) struct TableFuncAlias {
    /// テーブル名/エイリアス名 (オプション)
    /// 例: "t" in "t(i, v)", None in "(i, v)"
    col_id: Option<String>,
    /// カラム定義のリスト
    column_list: ColumnList,
    loc: Location,
}

impl TableFuncAlias {
    pub(crate) fn new(col_id: Option<String>, column_list: ColumnList, loc: Location) -> Self {
        Self {
            col_id,
            column_list,
            loc,
        }
    }

    /// TableFuncElementList を受け取り、内部的に ElementList に変換して保持する
    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        self.column_list.is_multi_line()
    }

    pub(crate) fn last_line_len(&self, depth: usize) -> usize {
        let col_id_len = self.col_id.as_ref().map(|id| id.len()).unwrap_or(0);

        if self.is_multi_line() {
            self.column_list.last_line_len(depth)
        } else {
            col_id_len + self.column_list.last_line_len(depth)
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        if let Some(col_id) = &self.col_id {
            result.push_str(col_id);
        }

        let rendered_list = self.column_list.render(depth)?;

        result.push_str(&rendered_list);

        Ok(result)
    }
}
