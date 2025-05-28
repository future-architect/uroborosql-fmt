use crate::{
    cst::{
        add_indent, AlignedExpr, Clause, ColumnList, Comment, ConflictTargetColumnList, Expr,
        Location, Statement,
    },
    error::UroboroSQLFmtError,
    util::{add_single_space, add_space_by_range, tab_size},
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
        add_indent(&mut result, depth);
        // CONSTRAINT
        result.push_str(&self.keyword.1);
        add_single_space(&mut result);
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

        add_indent(&mut result, depth - 1);
        // DO
        result.push_str(&self.keyword.0);
        result.push('\n');
        add_indent(&mut result, depth);
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

        add_indent(&mut result, depth - 1);
        // DO
        result.push_str(&self.keyword.0);
        result.push('\n');
        add_indent(&mut result, depth);
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
#[allow(clippy::large_enum_variant)]
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

        add_indent(&mut result, depth - 1);
        // ON
        result.push_str(&self.keyword.0);
        result.push('\n');
        add_indent(&mut result, depth);
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
                    add_single_space(&mut result);
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

/// INSERTにおけるVALUES句を格納
#[derive(Debug, Clone)]
pub(crate) struct Values {
    kw: String,
    rows: Vec<ColumnList>,
}

impl Values {
    pub(crate) fn new(kw: &str, rows: Vec<ColumnList>) -> Values {
        Values {
            kw: kw.to_string(),
            rows,
        }
    }

    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // VALUES句
        result.push_str(&self.kw);

        // 要素が一つか二つ以上かでフォーマット方針が異なる
        let is_one_row = self.rows.len() == 1;

        if !is_one_row {
            result.push('\n');
            add_indent(&mut result, depth);
        } else {
            // "VALUES" と "(" の間の空白
            result.push(' ');
        }

        let mut separator = String::from('\n');
        add_indent(&mut separator, depth - 1);
        separator.push(',');
        add_space_by_range(&mut separator, 1, tab_size());

        result.push_str(
            &self
                .rows
                .iter()
                .map(|cols| cols.render(depth - 1))
                .collect::<Result<Vec<_>, _>>()?
                .join(&separator),
        );
        result.push('\n');

        Ok(result)
    }
}

/// INSERT句におけるクエリを格納
#[derive(Debug, Clone)]
pub(crate) enum Query {
    Normal(Statement),
    Paren(Expr),
}

impl Query {
    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        result.push('\n');

        let formatted_query = match self {
            Query::Normal(normal) => normal.render(depth - 1)?,
            Query::Paren(paren) => paren.to_aligned().render(depth - 1)?,
        };
        result.push_str(&formatted_query);

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ValuesOrQuery {
    Values(Values),
    Query(Query),
}

impl ValuesOrQuery {
    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            ValuesOrQuery::Values(values) => values.render(depth),
            ValuesOrQuery::Query(query) => query.render(depth),
        }
    }
}

/// INSERT文の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct InsertBody {
    loc: Location,
    table_name: AlignedExpr,
    columns: Option<SeparatedLines>,
    values_or_query: Option<ValuesOrQuery>,
    on_conflict: Option<OnConflict>,
}

impl InsertBody {
    pub(crate) fn new(loc: Location, table_name: AlignedExpr) -> InsertBody {
        InsertBody {
            loc,
            table_name,
            columns: None,
            values_or_query: None,
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
        let values = Values::new(kw, body);
        self.values_or_query = Some(ValuesOrQuery::Values(values));
    }

    /// SELECT文をセットする
    pub(crate) fn set_query(&mut self, query: Statement) {
        self.values_or_query = Some(ValuesOrQuery::Query(Query::Normal(query)))
    }

    /// 括弧付きSELECTをセットする
    pub(crate) fn set_paren_query(&mut self, query: Expr) {
        self.values_or_query = Some(ValuesOrQuery::Query(Query::Paren(query)))
    }

    /// 直接 ValuesOrQuery をセットする
    pub(crate) fn set_values_or_query(&mut self, values_or_query: ValuesOrQuery) {
        self.values_or_query = Some(values_or_query);
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
        // 1. on_conflict
        // 2. values_or_query
        // 3. columns
        // else: table_name

        if self.on_conflict.is_some() {
            // on_conflict 句の後にコメントが来る場合
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "add_comment_to_child(): Comments after on_conflict clause is not implemented: {comment:?}"
            )));
        } else if let Some(values_or_query) = self.values_or_query.as_mut() {
            // values 句 か query の後にコメントが来る場合
            match values_or_query {
                ValuesOrQuery::Values(_) => {
                    // values 句の場合
                    return Err(UroboroSQLFmtError::Unimplemented(format!(
                        "add_comment_to_child(): Comments after values_clause are not implemented: {comment:?}"
                    )));
                }
                ValuesOrQuery::Query(query) => match query {
                    Query::Normal(statement) => {
                        // select 文のあとにコメントが来る場合
                        statement.add_comment_to_child(comment)?;
                    }
                    Query::Paren(_) => {
                        // 括弧付き select で、閉じ括弧の後にコメントが来る場合
                        return Err(UroboroSQLFmtError::Unimplemented(format!(
                            "add_comment_to_child(): Comments after select queries enclosed in parentheses are not implemented: {comment:?}"
                        )));
                    }
                },
            }
        } else if self.columns.is_some() {
            // カラム名の後 または columns の後にコメントが来る場合
            return Err(UroboroSQLFmtError::Unimplemented(format!(
                "add_comment_to_child(): Comments after column name is not implemented: {comment:?}"
            )));
        } else {
            // table_name 直後のコメント
            if comment.is_block_comment() {
                // ブロックコメントの場合
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "add_comment_to_child(): Block comment after table_name is not implemented: {comment:?}"
                )));
            } else if self.table_name.loc().is_same_line(&comment.loc()) {
                // 行末コメントである場合、table_nameに追加する
                self.table_name.set_trailing_comment(comment)?;
            } else {
                // それ以外は未対応
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "add_comment_to_child(): Comments for this location is not implemented: {comment:?}"
                )));
            }
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
        add_indent(&mut result, depth);
        result.push_str(&self.table_name.render(depth)?);
        result.push('\n');

        // カラム名
        if let Some(sep_lines) = &self.columns {
            add_indent(&mut result, depth - 1);
            result.push_str("(\n");
            result.push_str(&sep_lines.render(depth)?);
            add_indent(&mut result, depth - 1);
            result.push(')');

            // ValuesOrQuery が Values なら、 ')' の後にスペース(' ')を追加する
            if let Some(ValuesOrQuery::Values(_)) = &self.values_or_query {
                result.push(' ');
            }
        }

        if let Some(values_or_query) = &self.values_or_query {
            result.push_str(&values_or_query.render(depth)?);
        }

        if let Some(oc) = &self.on_conflict {
            result.push_str(&oc.render(depth)?);
        }

        Ok(result)
    }
}
