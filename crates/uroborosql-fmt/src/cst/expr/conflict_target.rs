use itertools::repeat_n;

use crate::{cst::Location, error::UroboroSQLFmtError};

/// COLLATE
#[derive(Debug, Clone)]
pub(crate) struct Collate {
    keyword: String,
    collation: String,
}

impl Collate {
    pub(crate) fn new(keyword: String, collation: String) -> Collate {
        Collate { keyword, collation }
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        result.push_str(&self.keyword);
        result.push('\t');
        result.push_str(&self.collation);
        Ok(result)
    }
}

/// insert文のconflict_targetにおけるカラムリストの要素
#[derive(Debug, Clone)]
pub(crate) struct ConflictTargetElement {
    column: String,
    collate: Option<Collate>,
    op_class: Option<String>,
}

impl ConflictTargetElement {
    pub(crate) fn new(column: String) -> ConflictTargetElement {
        ConflictTargetElement {
            column,
            collate: None,
            op_class: None,
        }
    }

    /// COLLATEのセット
    pub(crate) fn set_collate(&mut self, collate: Collate) {
        self.collate = Some(collate);
    }

    /// op_classのセット
    pub(crate) fn set_op_class(&mut self, op_class: String) {
        self.op_class = Some(op_class);
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        result.extend(repeat_n('\t', depth));
        result.push_str(&self.column);

        // collationがある場合
        if let Some(collate) = &self.collate {
            result.push('\t');
            result.push_str(&collate.render()?);
        };

        // op_classがある場合
        if let Some(op_class) = &self.op_class {
            result.push('\t');
            // 演算子クラスはキーワードルールを適用
            result.push_str(op_class);
        };

        Ok(result)
    }
}

/// insert文のconflict_targetにおけるカラムリスト
#[derive(Debug, Clone)]
pub(crate) struct ConflictTargetColumnList {
    cols: Vec<ConflictTargetElement>,
    /// Locationを示す
    /// 現状使用していないため_locとしている
    _loc: Location,
}

impl ConflictTargetColumnList {
    pub(crate) fn new(cols: Vec<ConflictTargetElement>, loc: Location) -> ConflictTargetColumnList {
        ConflictTargetColumnList { cols, _loc: loc }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 各列を複数行に出力する
        result.push_str("(\n");

        // 最初の行のインデント
        result.extend(repeat_n('\t', depth + 1));

        // 各要素間の改行、カンマ、インデント
        let mut separator = "\n".to_string();
        separator.extend(repeat_n('\t', depth));
        separator.push_str(",\t");

        result.push_str(
            &self
                .cols
                .iter()
                .map(|a| a.render(depth - 1))
                .collect::<Result<Vec<_>, _>>()?
                .join(&separator),
        );

        result.push('\n');
        result.extend(repeat_n('\t', depth));
        result.push(')');

        Ok(result)
    }
}
