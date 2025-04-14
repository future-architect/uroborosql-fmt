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
    right: AlignedExpr,

    // ON, USING
    qualifier: Option<Clause>,

    // 行末コメント
    trailing_comments: Vec<Comment>,
}

impl JoinedTable {
    pub(crate) fn new(
        loc: Location,
        left: AlignedExpr,
        join_keyword: String,
        right: AlignedExpr,
    ) -> Self {
        Self {
            loc,
            left,
            join_keyword,
            right,
            qualifier: None,
            trailing_comments: vec![],
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
        if let Some(qualifier) = &mut self.qualifier {
            qualifier.add_comment_to_child(comment)?;
        } else {
            self.trailing_comments.push(comment);
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

        // right
        add_indent(&mut result, depth);
        result.push_str(&self.right.render(depth)?);

        if let Some(qualifier) = &self.qualifier {
            result.push('\n');

            result.push_str(&qualifier.render(depth - 1)?);
        }

        Ok(result)
    }
}

impl From<JoinedTable> for Expr {
    fn from(joined_table: JoinedTable) -> Self {
        Expr::JoinedTable(Box::new(joined_table))
    }
}
