use crate::{
    cst::{add_indent, AlignedExpr, Comment, Expr, Location, OnConflict, Statement},
    error::UroboroSQLFmtError,
};

use super::separeted_lines::SeparatedLines;

/// INSERT句におけるクエリを格納
#[derive(Debug, Clone)]
pub(crate) enum Query {
    Normal(Statement),
    Paren(Expr),
}

impl Query {
    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let formatted_query = match self {
            Query::Normal(normal) => normal.render(depth - 1)?,
            Query::Paren(paren) => paren.to_aligned().render(depth - 1)?,
        };
        result.push_str(&formatted_query);

        Ok(result)
    }
}

/// INSERT文の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct PgInsertBody {
    loc: Location,
    table_name: AlignedExpr,
    columns: Option<SeparatedLines>,
    query: Option<Query>,
    on_conflict: Option<OnConflict>,
}

impl PgInsertBody {
    pub(crate) fn new(loc: Location, table_name: AlignedExpr) -> PgInsertBody {
        PgInsertBody {
            loc,
            table_name,
            columns: None,
            query: None,
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

    /// SELECT文をセットする
    pub(crate) fn set_query(&mut self, query: Statement) {
        self.query = Some(Query::Normal(query))
    }

    /// 括弧付きSELECTをセットする
    pub(crate) fn set_paren_query(&mut self, query: Expr) {
        self.query = Some(Query::Paren(query))
    }

    pub(crate) fn set_on_conflict(&mut self, on_conflict: OnConflict) {
        self.on_conflict = Some(on_conflict);
    }

    /// TODO: 子供にコメントを追加する
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
        // 2. query
        // 3. columns
        // else: table_name

        // とりあえず table_name だけ実装
        self.table_name.set_trailing_comment(comment)?;

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
        }

        if let Some(values_or_query) = &self.query {
            result.push(' ');
            result.push_str(&values_or_query.render(depth)?);
        }

        if let Some(oc) = &self.on_conflict {
            result.push_str(&oc.render(depth)?);
        }

        Ok(result)
    }
}
