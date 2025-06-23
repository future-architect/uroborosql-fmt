use crate::{
    cst::{from_list::TableRef, Comment, Location, SeparatedLines},
    error::UroboroSQLFmtError,
    util::add_indent,
};

use super::Expr;

#[derive(Debug, Clone)]
pub struct Qualifier {
    keyword: String,
    comments_after_keyword: Vec<Comment>,
    condition: SeparatedLines,
}

impl Qualifier {
    pub(crate) fn new(
        keyword: String,
        comments_after_keyword: Vec<Comment>,
        condition: SeparatedLines,
    ) -> Self {
        Self {
            keyword,
            comments_after_keyword,
            condition,
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        self.condition.add_comment_to_child(comment)?;

        Ok(())
    }

    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        let last_content = self.condition.last_content().unwrap();
        last_content.last_line_len_from_left(acc)
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        add_indent(&mut result, depth - 1);
        result.push_str(&self.keyword);
        result.push('\n');

        for comment in &self.comments_after_keyword {
            result.push_str(&comment.render(depth - 1)?);
            result.push('\n');
        }

        result.push_str(&self.condition.render(depth)?);
        // SeparatedLines を利用しているため末尾の改行を削除する
        assert_eq!(result.pop(), Some('\n'));

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JoinedTable {
    loc: Location,
    left: TableRef,
    comments_after_left: Vec<Comment>,
    join_keyword: String,
    comments_after_join_keyword: Vec<Comment>,
    right: TableRef,
    comments_after_right: Vec<Comment>,

    // ON, USING
    qualifier: Option<Qualifier>,
}

impl JoinedTable {
    pub(crate) fn new(
        loc: Location,
        left: TableRef,
        comments_after_left: Vec<Comment>,
        join_keyword: String,
        comments_after_join_keyword: Vec<Comment>,
        right: TableRef,
    ) -> Self {
        Self {
            loc,
            left,
            comments_after_left,
            join_keyword,
            comments_after_join_keyword,
            right,
            comments_after_right: vec![],
            qualifier: None,
        }
    }

    pub(crate) fn set_qualifier(&mut self, qualifier: Qualifier) {
        self.qualifier = Some(qualifier);
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        true
    }

    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        if let Some(qualifier) = &self.qualifier {
            qualifier.last_line_len_from_left(acc)
        } else {
            self.right.last_line_len_from_left(acc)
        }
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        self.left.set_head_comment(comment);
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // qualifier があればその下に追加する
        if let Some(qualifier) = &mut self.qualifier {
            qualifier.add_comment_to_child(comment)?;
        } else if !comment.is_block_comment() && comment.loc().is_same_line(&self.right.loc()) {
            //  qualifier が無く、右辺の末尾コメントであれば右辺に追加する
            self.right.set_trailing_comment(comment)?;
        } else {
            // 右辺の末尾コメントでもない場合は右辺の後のコメントになる
            self.comments_after_right.push(comment);
        }

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // left
        result.push_str(&self.left.render(depth)?);
        result.push('\n');

        for comment in &self.comments_after_left {
            result.push_str(&comment.render(depth - 1)?);
            result.push('\n');
        }

        // join keyword
        add_indent(&mut result, depth - 1);
        result.push_str(&self.join_keyword);
        result.push('\n');

        for comment in &self.comments_after_join_keyword {
            result.push_str(&comment.render(depth - 1)?);
            result.push('\n');
        }

        // right
        add_indent(&mut result, depth);
        result.push_str(&self.right.render(depth)?);

        for comment in &self.comments_after_right {
            result.push('\n');
            result.push_str(&comment.render(depth - 1)?);
        }

        if let Some(qualifier) = &self.qualifier {
            result.push('\n');
            result.push_str(&qualifier.render(depth)?);
        }

        Ok(result)
    }
}

impl From<JoinedTable> for Expr {
    fn from(joined_table: JoinedTable) -> Self {
        Expr::JoinedTable(Box::new(joined_table))
    }
}
