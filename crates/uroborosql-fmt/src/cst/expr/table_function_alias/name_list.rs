use crate::{
    cst::{ColumnList, Comment, Expr, Location, PrimaryExpr},
    error::UroboroSQLFmtError,
};
pub(crate) struct Name {
    name: PrimaryExpr,
    trailing_comment: Option<Comment>,
}

impl Name {
    pub(crate) fn new(name: PrimaryExpr) -> Self {
        Self {
            name,
            trailing_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.name.loc()
    }

    /// 行末コメントを設定する
    /// 呼び出し側でコメントと要素が同一行にあることを保証する
    pub(crate) fn set_trailing_comment(&mut self, comment: Comment) {
        self.trailing_comment = Some(comment);
    }
}

pub(crate) struct ParenthesizedNameList {
    names: Vec<Name>,
    loc: Location,
    start_comments: Vec<Comment>,
}

impl ParenthesizedNameList {
    pub(crate) fn new(names: Vec<Name>, loc: Location, start_comments: Vec<Comment>) -> Self {
        Self {
            start_comments,
            names,
            loc,
        }
    }

    /// ParenthesizedNameList を ColumnList に変換する
    pub(crate) fn try_into_column_list(self) -> Result<ColumnList, UroboroSQLFmtError> {
        // Vec<Name> を Vec<AlignedExpr> に変換する
        let cols = self
            .names
            .iter()
            .map(|element| {
                let mut aligned = Expr::Primary(Box::new(element.name.clone())).to_aligned();

                if let Some(trailing_comment) = element.trailing_comment.clone() {
                    aligned.set_trailing_comment(trailing_comment)?;
                }

                Ok(aligned)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ColumnList::new(cols, self.loc, self.start_comments))
    }
}

impl From<ParenthesizedNameList> for ColumnList {
    fn from(list: ParenthesizedNameList) -> Self {
        // Vec<Name> を Vec<AlignedExpr> に変換する
        let cols = list
            .names
            .iter()
            .map(|element| {
                let mut aligned = Expr::Primary(Box::new(element.name.clone())).to_aligned();

                if let Some(trailing_comment) = element.trailing_comment.clone() {
                    // Name が持つ行末コメントは要素と同一行にあることがすでに保証されているため失敗しない
                    aligned
                        .set_trailing_comment(trailing_comment)
                        .expect("Setting trailing comment cannot fail because the Name's trailing comment is already guaranteed to be on the same line as its element.");
                }

                aligned
            })
            .collect::<Vec<_>>();

        ColumnList::new(cols, list.loc, list.start_comments)
    }
}
