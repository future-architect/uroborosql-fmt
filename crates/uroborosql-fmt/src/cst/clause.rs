use crate::{
    error::UroboroSQLFmtError,
    util::{add_single_space, convert_keyword_case},
};

use super::{add_indent, Body, Comment, Location, SqlID};

// 句に対応した構造体
#[derive(Debug, Clone)]
pub(crate) struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<Body>,
    loc: Location,
    /// DML(, DDL)に付与できるsql_id
    sql_id: Option<SqlID>,
    /// キーワードの下に現れるコメント
    comments: Vec<Comment>,
}

impl Clause {
    /// NodeからClauseを生成する
    /// キーワードの大文字小文字を設定に合わせて自動で変換する
    pub(crate) fn from_pg_node(kw_node: postgresql_cst_parser::tree_sitter::Node) -> Clause {
        let keyword = convert_keyword_case(kw_node.text());
        let loc = Location::from(kw_node.range());
        Clause {
            keyword,
            body: None,
            loc,
            sql_id: None,
            comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn keyword(&self) -> String {
        self.keyword.clone()
    }

    /// postgresql-cst-parser の Node でキーワードを延長する (延長にはスペースを使用)
    /// この時、キーワードの大文字小文字を設定に合わせて自動で変換する
    pub(crate) fn pg_extend_kw(&mut self, node: postgresql_cst_parser::tree_sitter::Node) {
        let loc = Location::from(node.range());
        self.loc.append(loc);
        self.keyword.push(' ');
        self.keyword.push_str(&convert_keyword_case(node.text()));
    }

    /// 文字列を受け取ってキーワードを延長する
    /// この時、キーワードの大文字小文字を設定に合わせて自動で変換する
    pub(crate) fn extend_kw_with_string(&mut self, kw: &str) {
        self.keyword.push(' ');
        self.keyword.push_str(&convert_keyword_case(kw));
    }

    /// bodyをセットする
    pub(crate) fn set_body(&mut self, body: Body) {
        if !body.is_empty() {
            self.loc.append(body.loc().unwrap());
            self.body = Some(body);
        }

        self.fix_head_comment();
    }

    /// Clauseにコメントを追加する
    /// Bodyがあればその下にコメントを追加し、ない場合はキーワードの下にコメントを追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match &mut self.body {
            Some(body) if !body.is_empty() => {
                // bodyに式があれば、その下につく
                body.add_comment_to_child(comment)?;
            }
            _ => {
                // そうでない場合、自分のキーワードの下につく
                self.comments.push(comment);
            }
        }

        Ok(())
    }

    /// SQL_IDをセットする
    pub(crate) fn set_sql_id(&mut self, sql_id: SqlID) {
        self.sql_id = Some(sql_id);
    }

    /// Clause のキーワードの下のコメントとして追加したコメントが、
    /// バインドパラメータである場合に、式のバインドパラメータとして付け替えるメソッド。
    ///
    /// 例えば、以下のSQLの `/*param*/` は Clause のコメントとして扱われているため、
    /// 式 `1` のバインドパラメータとして付け替える必要がある。
    /// ```sql
    /// THEN
    ///     /*param*/1
    /// ```
    fn fix_head_comment(&mut self) {
        if let Some(last_comment) = self.comments.last() {
            if let Some(body) = &mut self.body {
                if body.try_set_head_comment(last_comment.clone()) {
                    self.comments.pop();
                }
            }
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // kw
        // body...
        let mut result = String::new();

        add_indent(&mut result, depth);
        result.push_str(&self.keyword);

        if let Some(sql_id) = &self.sql_id {
            result.push(' ');
            result.push_str(&sql_id.sql_id);
        }

        // comments
        for comment in &self.comments {
            result.push('\n');
            result.push_str(&comment.render(depth)?);
        }

        match &self.body {
            // 句と本体を同じ行に render する
            Some(Body::SingleLine(single_line)) => {
                add_single_space(&mut result);
                result.push_str(&single_line.render(depth)?);
            }
            Some(body) => {
                let formatted_body = body.render(depth + 1)?;
                result.push('\n');
                result.push_str(&formatted_body);
            }
            None => result.push('\n'),
        }

        Ok(result)
    }
}
