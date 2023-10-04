use itertools::repeat_n;

use crate::{
    cst::{ColumnList, Comment, Location, SubExpr},
    error::UroboroSQLFmtError,
};

/// WITH句における名前付きサブクエリ}
/// cte (Common Table Expressions)
#[derive(Debug, Clone)]
pub(crate) struct Cte {
    loc: Location,
    name: String,
    as_keyword: String,
    column_name: Option<ColumnList>,
    materialized_keyword: Option<String>,
    sub_expr: SubExpr,
    /// 行末コメント
    trailing_comment: Option<String>,
    /// テーブル名の直後に現れる行末コメント
    name_trailing_comment: Option<String>,
}

impl Cte {
    pub(crate) fn new(
        loc: Location,
        name: String,
        as_keyword: String,
        column_name: Option<ColumnList>,
        materialized_keyword: Option<String>,
        statement: SubExpr,
    ) -> Cte {
        Cte {
            loc,
            name,
            as_keyword,
            column_name,
            materialized_keyword,
            sub_expr: statement,
            trailing_comment: None,
            name_trailing_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// cteのtrailing_commentをセットする
    /// 複数行コメントを与えた場合エラーを返す
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_trailing_comment:{comment:?} is not trailing comment!"
            )))
        } else {
            let Comment { text, loc } = comment;
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// テーブル名のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_name_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperation(format!(
                "set_name_trailing_comment:{comment:?} is not trailing comment!"
            )))
        } else {
            // 行コメント
            let Comment { text, loc } = comment;
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());
            self.name_trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.name);
        result.push('\t');

        // カラム名の指定がある場合
        if let Some(column_list) = &self.column_name {
            result.push_str(&column_list.render(depth)?);
            result.push('\t');
        }

        // テーブル名の直後のコメントがある場合
        if let Some(comment) = &self.name_trailing_comment {
            result.push_str(comment);
            result.push('\n');
            result.extend(repeat_n('\t', depth));
        }

        result.push_str(&self.as_keyword);
        result.push('\t');

        // MATERIALIZEDの指定がある場合
        if let Some(materialized) = &self.materialized_keyword {
            result.push_str(materialized);
            result.push('\t');
        }

        result.push_str(&self.sub_expr.render(depth)?);

        if let Some(comment) = &self.trailing_comment {
            result.push('\t');
            result.push_str(comment);
        }

        Ok(result)
    }
}

/// WITH句の本体。
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct WithBody {
    loc: Option<Location>,
    contents: Vec<(Cte, Vec<Comment>)>,
}

impl WithBody {
    pub(crate) fn new() -> WithBody {
        WithBody {
            loc: None,
            contents: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // cteを追加する
    pub(crate) fn add_cte(&mut self, cte: Cte) {
        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(cte.loc()),
            None => self.loc = Some(cte.loc()),
        };

        self.contents.push((cte, vec![]));
    }

    /// 最後のcteにコメントを追加する
    /// 最後のcteと同じ行である場合は行末コメントとして追加し、そうでない場合はcteの下のコメントとして追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if comment.is_block_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            // 最後の要素にコメントを追加
            self.contents.last_mut().unwrap().1.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.contents
                .last_mut()
                .unwrap()
                .0
                .set_trailing_comment(comment)?;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(comment_loc),
            None => self.loc = Some(comment_loc),
        };

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let mut is_first_line = true;

        for (cte, comments) in &self.contents {
            result.extend(repeat_n('\t', depth - 1));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push(',')
            }
            result.push('\t');

            let formatted = cte.render(depth)?;
            result.push_str(&formatted);
            result.push('\n');

            // commentsのrender
            for comment in comments {
                result.push_str(&comment.render(depth - 1)?);
                result.push('\n');
            }
        }
        Ok(result)
    }
}
