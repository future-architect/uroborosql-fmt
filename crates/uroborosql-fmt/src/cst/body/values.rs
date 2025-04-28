use crate::{
    cst::{ColumnList, Comment, Location},
    error::UroboroSQLFmtError,
    util::{add_indent, add_space_by_range, tab_size},
};

#[derive(Debug, Clone)]
pub(crate) struct ValuesBody {
    loc: Location,
    rows: Vec<ColumnList>,
}

impl ValuesBody {
    pub(crate) fn new(loc: Location, rows: Vec<ColumnList>) -> ValuesBody {
        ValuesBody { loc, rows }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 要素が一つか二つ以上かでフォーマット方針が異なる
        let is_one_row = self.rows.len() == 1;

        if !is_one_row {
            result.push('\n');
            add_indent(&mut result, depth + 1);
        } else {
            // "VALUES" と "(" の間の空白
            result.push(' ');
        }

        let mut separator = String::from('\n');
        add_indent(&mut separator, depth);
        separator.push(',');
        add_space_by_range(&mut separator, 1, tab_size());

        result.push_str(
            &self
                .rows
                .iter()
                .map(|cols| cols.render(depth))
                .collect::<Result<Vec<_>, _>>()?
                .join(&separator),
        );
        result.push('\n');

        Ok(result)
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        unimplemented!()
    }

    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        unimplemented!()
    }
}
