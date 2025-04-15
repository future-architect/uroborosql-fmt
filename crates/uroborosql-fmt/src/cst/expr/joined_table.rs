use crate::{
    cst::{AlignedExpr, Clause, Comment, Location},
    error::UroboroSQLFmtError,
    util::add_indent,
};

use super::Expr;

#[derive(Debug, Clone)]
pub(crate) struct JoinedTable {
    loc: Location,
    left: AlignedExpr,
    join_keyword: String,
    comments_after_join_keyword: Vec<Comment>,
    right: AlignedExpr,

    // ON, USING
    qualifier: Option<Clause>,
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

    pub(crate) fn set_qualifier(&mut self, qualifier: Clause) {
        self.qualifier = Some(qualifier);
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn is_multi_line(&self) -> bool {
        true
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // qualifier があればその下に追加する
        if let Some(qualifier) = &mut self.qualifier {
            qualifier.add_comment_to_child(comment)?;
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

            result.push_str(&qualifier.render(depth - 1)?);

            // Clause の末尾には改行が含まれるが、JoinedTable の末尾では改行しないようにするため除外する
            let last_char = result.pop();
            assert_eq!(last_char, Some('\n'));
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
