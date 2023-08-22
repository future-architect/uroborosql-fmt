use itertools::{repeat_n, Itertools};

use crate::{
    cst::{AlignedExpr, Clause, ColumnList, Comment, ConflictTargetColumnList, Location},
    error::UroboroSQLFmtError,
};

use super::separeted_lines::SeparatedLines;

/// INSERT文のconflict_targetにおいてindexカラムを指定した場合
#[derive(Debug, Clone)]
pub(crate) struct SpecifyIndexColumn {
    index_expression: ConflictTargetColumnList,
    where_clause: Option<Clause>,
}

impl SpecifyIndexColumn {
    pub(crate) fn new(index_expression: ConflictTargetColumnList) -> SpecifyIndexColumn {
        SpecifyIndexColumn {
            index_expression,
            where_clause: None,
        }
    }

    /// where句の追加
    pub(crate) fn set_where_clause(&mut self, where_clause: Clause) {
        self.where_clause = Some(where_clause);
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.index_expression.render(depth)?);
        result.push('\n');

        if let Some(where_clause) = &self.where_clause {
            result.push_str(&where_clause.render(depth - 1)?);
        }

        Ok(result)
    }
}

/// INSERT文のconflict_targetにおけるON CONSTRAINT
#[derive(Debug, Clone)]
pub(crate) struct OnConstraint {
    /// (ON, CONSTRAINT)
    keyword: (String, String),
    constraint_name: String,
}

impl OnConstraint {
    pub(crate) fn new(keyword: (String, String), constraint_name: String) -> OnConstraint {
        OnConstraint {
            keyword,
            constraint_name,
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        // ON
        result.push_str(&self.keyword.0);
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // CONSTRAINT
        result.push_str(&self.keyword.1);
        result.push('\t');
        result.push_str(&self.constraint_name);
        result.push('\n');

        Ok(result)
    }
}

/// INSERT文におけるconflict_target
#[derive(Debug, Clone)]
pub(crate) enum ConflictTarget {
    SpecifyIndexColumn(SpecifyIndexColumn),
    OnConstraint(OnConstraint),
}

/// INSERT文のconflict_actionにおけるDO NOTHING
#[derive(Debug, Clone)]
pub(crate) struct DoNothing {
    /// (DO, NOTHING)
    keyword: (String, String),
}

impl DoNothing {
    pub(crate) fn new(keyword: (String, String)) -> DoNothing {
        DoNothing { keyword }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // DO
        result.push_str(&self.keyword.0);
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // NOTHING
        result.push_str(&self.keyword.1);
        result.push('\n');

        Ok(result)
    }
}

/// INSERT文のconflict_actionにおけるDO UPDATE
#[derive(Debug, Clone)]
pub(crate) struct DoUpdate {
    /// (DO, UPDATE)
    keyword: (String, String),
    set_clause: Clause,
    where_clause: Option<Clause>,
}

impl DoUpdate {
    pub(crate) fn new(
        keyword: (String, String),
        set_clause: Clause,
        where_clause: Option<Clause>,
    ) -> DoUpdate {
        DoUpdate {
            keyword,
            set_clause,
            where_clause,
        }
    }
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // DO
        result.push_str(&self.keyword.0);
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // UPDATE
        result.push_str(&self.keyword.1);
        result.push('\n');
        // SET句
        result.push_str(&self.set_clause.render(depth)?);
        // WHERE句
        if let Some(where_clause) = &self.where_clause {
            result.push_str(&where_clause.render(depth)?);
        }

        Ok(result)
    }
}

/// INSERT文におけるconflict_action
#[derive(Debug, Clone)]
pub(crate) enum ConflictAction {
    DoNothing(DoNothing),
    DoUpdate(DoUpdate),
}

/// INSERT文におけるON CONFLICT
#[derive(Debug, Clone)]
pub(crate) struct OnConflict {
    /// (ON CONFLICT)
    keyword: (String, String),
    conflict_target: Option<ConflictTarget>,
    conflict_action: ConflictAction,
}

impl OnConflict {
    pub(crate) fn new(
        keyword: (String, String),
        conflict_target: Option<ConflictTarget>,
        conflict_action: ConflictAction,
    ) -> OnConflict {
        OnConflict {
            keyword,
            conflict_target,
            conflict_action,
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.extend(repeat_n('\t', depth - 1));
        // ON
        result.push_str(&self.keyword.0);
        result.push('\n');
        result.extend(repeat_n('\t', depth));
        // CONFLICT
        result.push_str(&self.keyword.1);

        if let Some(conflict_target) = &self.conflict_target {
            match conflict_target {
                ConflictTarget::OnConstraint(on_constraint) => {
                    // ON CONSTRAINTの場合は改行して描画
                    result.push('\n');
                    result.push_str(&on_constraint.render(depth)?);
                }
                ConflictTarget::SpecifyIndexColumn(specify_index_column) => {
                    // INDEXカラム指定の場合は改行せずに描画
                    result.push('\t');
                    result.push_str(&specify_index_column.render(depth)?);
                }
            }
        } else {
            // conflict_targetがない場合は改行
            result.push('\n');
        }

        match &self.conflict_action {
            ConflictAction::DoNothing(do_nothing) => result.push_str(&do_nothing.render(depth)?),
            ConflictAction::DoUpdate(do_update) => result.push_str(&do_update.render(depth)?),
        }

        Ok(result)
    }
}

/// INSERT文の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct InsertBody {
    loc: Location,
    table_name: AlignedExpr,
    columns: Option<SeparatedLines>,
    values_kw: Option<String>,
    values_rows: Vec<ColumnList>,
    on_conflict: Option<OnConflict>,
}

impl InsertBody {
    pub(crate) fn new(loc: Location, table_name: AlignedExpr) -> InsertBody {
        InsertBody {
            loc,
            table_name,
            columns: None,
            values_kw: None,
            values_rows: vec![],
            on_conflict: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// カラム名をセットする
    pub(crate) fn set_column_name(&mut self, cols: SeparatedLines) {
        self.columns = Some(cols);
    }

    /// VALUES句をセットする
    pub(crate) fn set_values_clause(&mut self, kw: &str, body: Vec<ColumnList>) {
        self.values_kw = Some(kw.to_string());
        self.values_rows = body;
    }

    pub(crate) fn set_on_conflict(&mut self, on_conflict: OnConflict) {
        self.on_conflict = Some(on_conflict);
    }

    /// 子供にコメントを追加する
    ///
    /// 対応済み
    /// - テーブル名の行末コメント
    ///
    /// 未対応
    /// - VALUES句の直後に現れるコメント
    /// - VALUES句の本体に現れるコメント
    /// - カラム名の直後に現れるコメント
    /// - テーブル名の直後に現れるコメント
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // 下から順番に見ていく

        // table_nameの直後に現れる
        if comment.is_block_comment() || !self.table_name.loc().is_same_line(&comment.loc()) {
            // 行末コメントではない場合は未対応
            unimplemented!()
        } else {
            // 行末コメントである場合、table_nameに追加する
            self.table_name.set_trailing_comment(comment)?;
        }

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は INSERT が描画される行のインデントの深さ + 1 (つまり、テーブル名が描画される行の深さ)
        if depth < 1 {
            // インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::Rendering(
                "InsertBody::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();

        // テーブル名
        result.extend(repeat_n('\t', depth));
        result.push_str(&self.table_name.render(depth)?);
        result.push('\n');

        // カラム名
        if let Some(sep_lines) = &self.columns {
            result.extend(repeat_n('\t', depth - 1));
            result.push_str("(\n");
            result.push_str(&sep_lines.render(depth)?);
            result.extend(repeat_n('\t', depth - 1));
            result.push(')');
        }

        // VALUES句
        if let Some(kw) = &self.values_kw {
            result.push(' ');
            result.push_str(kw);

            // 要素が一つか二つ以上かでフォーマット方針が異なる
            let is_one_row = self.values_rows.len() == 1;

            if !is_one_row {
                result.push('\n');
                result.extend(repeat_n('\t', depth));
            } else {
                // "VALUES" と "(" の間の空白
                result.push(' ');
            }

            let mut separator = String::from('\n');
            separator.extend(repeat_n('\t', depth - 1));
            separator.push_str(",\t");

            result.push_str(
                &self
                    .values_rows
                    .iter()
                    .filter_map(|cols| cols.render(depth - 1).ok())
                    .join(&separator),
            );
            result.push('\n');
        } else {
            // VALUES句があるときは、改行を入れずに`VALUES`キーワードを出力している
            // そのため、VALUES句がない場合はここで改行
            result.push('\n');
        }

        if let Some(oc) = &self.on_conflict {
            result.push_str(&oc.render(depth)?);
        }

        Ok(result)
    }
}
