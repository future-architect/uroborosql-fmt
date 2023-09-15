use crate::{
    cst::{Clause, Comment, Location},
    error::UroboroSQLFmtError,
};

use super::Body;

/// SELECT句の本体
#[derive(Debug, Clone)]
pub(crate) struct SelectBody {
    loc: Option<Location>,
    all_distinct: Option<Clause>,
    select_clause_body: Option<Body>,
}

impl SelectBody {
    pub(crate) fn new() -> SelectBody {
        Self {
            loc: None,
            all_distinct: None,
            select_clause_body: None,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    pub(crate) fn set_all_distinct(&mut self, all_distinct: Clause) {
        self.loc = Some(all_distinct.loc());
        self.all_distinct = Some(all_distinct);
    }

    pub(crate) fn set_select_clause_body(&mut self, select_clause_body: Body) {
        // select_clause_bodyのlocが存在する場合はlocを更新
        if let Some(select_clause_body_loc) = select_clause_body.loc() {
            if let Some(loc) = &mut self.loc {
                // すでにlocが存在する(= ALL|DISTINCTが存在する)場合は既存のlocを拡張
                loc.append(select_clause_body_loc);
            } else {
                self.loc = Some(select_clause_body_loc);
            }
        }

        self.select_clause_body = Some(select_clause_body);
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if let Some(select_clause_body) = &mut self.select_clause_body {
            select_clause_body.add_comment_to_child(comment)?
        }

        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.all_distinct.is_none() && self.select_clause_body.is_none()
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut res = String::new();

        if let Some(all_distinct) = &self.all_distinct {
            res.push_str(&all_distinct.render(depth)?);
        }

        if let Some(select_clause_body) = &self.select_clause_body {
            res.push_str(&select_clause_body.render(depth)?);
        }

        Ok(res)
    }
}
