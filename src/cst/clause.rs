use itertools::repeat_n;
use tree_sitter::Node;

use crate::util::convert_keyword_case;

use super::{Body, Comment, Location, UroboroSQLFmtError};

// 句に対応した構造体
#[derive(Debug, Clone)]
pub(crate) struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<Body>,
    loc: Location,
    /// DML(, DDL)に付与できるsql_id
    sql_id: Option<Comment>,
    /// キーワードの下に現れるコメント
    comments: Vec<Comment>,
}

impl Clause {
    pub(crate) fn new(kw_node: Node, src: &str) -> Clause {
        let keyword = convert_keyword_case(kw_node.utf8_text(src.as_bytes()).unwrap());
        let loc = Location::new(kw_node.range());
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

    /// キーワードを延長する
    pub(crate) fn extend_kw(&mut self, node: Node, src: &str) {
        let loc = Location::new(node.range());
        self.loc.append(loc);
        self.keyword.push(' ');
        self.keyword.push_str(&convert_keyword_case(
            node.utf8_text(src.as_bytes()).unwrap(),
        ));
    }

    /// 文字列を受け取ってキーワードを延長する
    pub(crate) fn extend_kw_with_string(&mut self, kw: &str) {
        self.keyword.push(' ');
        self.keyword.push_str(&convert_keyword_case(kw));
    }

    /// タブ文字でキーワードを延長する
    pub(crate) fn extend_kw_with_tab(&mut self, node: Node, src: &str) {
        let loc = Location::new(node.range());
        self.loc.append(loc);
        self.keyword.push('\t');
        self.keyword.push_str(&convert_keyword_case(
            node.utf8_text(src.as_bytes()).unwrap(),
        ));
    }

    // bodyをセットする
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

    pub(crate) fn set_sql_id(&mut self, comment: Comment) {
        self.sql_id = Some(comment);
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

        result.extend(repeat_n('\t', depth));
        result.push_str(&self.keyword);

        if let Some(sql_id) = &self.sql_id {
            result.push(' ');
            result.push_str(&sql_id.text);
        }

        // comments
        for comment in &self.comments {
            result.push('\n');
            result.push_str(&comment.render(depth)?);
        }

        match &self.body {
            // 句と本体を同じ行に render する
            Some(Body::SingleLine(single_line)) => {
                result.push('\t');
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
