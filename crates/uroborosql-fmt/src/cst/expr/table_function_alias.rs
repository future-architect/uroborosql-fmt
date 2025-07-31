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
    table_func_element_list: ColumnList,
    loc: Location,
}

impl TableFuncAlias {
    pub(crate) fn new(
        col_id: Option<String>,
        table_func_element_list: ColumnList,
        loc: Location,
    ) -> TableFuncAlias {
        TableFuncAlias {
            col_id,
            table_func_element_list,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_len(&self, acc: usize) -> usize {
        // カラム定義が複数行である場合
        if self.table_func_element_list.is_multi_line() {
            ")".len()
        } else {
            // それ以外はカラム定義の長さを加算
            if let Some(col_id) = &self.col_id {
                acc + col_id.len() + self.table_func_element_list.last_line_len(acc)
            } else {
                acc + self.table_func_element_list.last_line_len(acc)
            }
        }
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        // カラム定義の設定値に依存する
        self.table_func_element_list.is_multi_line()
    }

    /// テーブル関数エイリアス句をrenderする。
    /// 自身の is_multi_line() が true になる場合には複数行で描画し、false になる場合単一行で描画する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は開きかっこを描画する行のインデントの深さ
        let mut result = String::new();

        // col_id がある場合、最初に描画
        if let Some(col_id) = &self.col_id {
            result.push_str(col_id);
        }

        result.push_str(&self.table_func_element_list.render(depth)?);

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}
