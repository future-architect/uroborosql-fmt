use crate::{
    cst::{AlignedExpr, Comment, Location, SeparatedLines},
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
    left: AlignedExpr,
    join_keyword: String,
    comments_after_join_keyword: Vec<Comment>,
    right: AlignedExpr,

    // ON, USING
    qualifier: Option<Qualifier>,
    end_comments: Vec<Comment>,
}

impl JoinedTable {
    pub(crate) fn new(
        loc: Location,
        left: AlignedExpr,
        join_keyword: String,
        comments_after_join_keyword: Vec<Comment>,
        right: AlignedExpr,
    ) -> Self {
        Self {
            loc,
            left,
            join_keyword,
            comments_after_join_keyword,
            right,
            qualifier: None,
            end_comments: vec![],
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

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // qualifier があればその下に追加する
        if let Some(qualifier) = &mut self.qualifier {
            qualifier.add_comment_to_child(comment)?;
        } else if !comment.is_block_comment() && comment.loc().is_same_line(&self.right.loc()) {
            //  右辺の末尾コメントであれば右辺に追加する
            self.right.set_trailing_comment(comment)?;
        } else {
            self.end_comments.push(comment);
        }

        Ok(())
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // left
        result.push_str(&self.left.render(depth)?);
        result.push('\n');

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

        if let Some(qualifier) = &self.qualifier {
            result.push('\n');
            result.push_str(&qualifier.render(depth)?);
        }

        for comment in &self.end_comments {
            result.push('\n');
            result.push_str(&comment.render(depth)?);
        }

        Ok(result)
    }
}

impl From<JoinedTable> for Expr {
    fn from(joined_table: JoinedTable) -> Self {
        Expr::JoinedTable(Box::new(joined_table))
    }
}
