use crate::cst::{AlignedExpr, ColumnList, Comment, Expr, Location, PrimaryExpr};

#[derive(Debug, Clone)]
pub(crate) struct ParenthesizedTableFuncElementList {
    elements: Vec<TableFuncElement>,
    loc: Location,
    start_comments: Vec<Comment>,
}

impl ParenthesizedTableFuncElementList {
    pub(crate) fn new(
        elements: Vec<TableFuncElement>,
        loc: Location,
        start_comments: Vec<Comment>,
    ) -> Self {
        Self {
            elements,
            loc,
            start_comments,
        }
    }
}

impl From<ParenthesizedTableFuncElementList> for ColumnList {
    fn from(list: ParenthesizedTableFuncElementList) -> Self {
        let exprs = list
            .elements
            .into_iter()
            .map(AlignedExpr::from)
            .collect::<Vec<_>>();

        ColumnList::new(exprs, list.loc, list.start_comments)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TableFuncElement {
    column_name: PrimaryExpr,
    column_type: PrimaryExpr,
    trailing_comment: Option<Comment>,
    loc: Location,
}

impl TableFuncElement {
    pub(crate) fn new(column_name: PrimaryExpr, column_type: PrimaryExpr, loc: Location) -> Self {
        Self {
            column_name,
            column_type,
            trailing_comment: None,
            loc,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 行末コメントを設定する
    /// 呼び出し側でコメントと要素が同一行にあることを保証する
    pub(crate) fn set_trailing_comment(&mut self, comment: Comment) {
        self.trailing_comment = Some(comment);
    }
}

impl From<TableFuncElement> for AlignedExpr {
    fn from(element: TableFuncElement) -> Self {
        let mut aligned = Expr::Primary(Box::new(element.column_name)).to_aligned();
        aligned.add_rhs(None, element.column_type.clone().into());

        if let Some(trailing_comment) = element.trailing_comment {
            // TableFuncElement が持つ行末コメントは要素と同一行にあることがすでに保証されているため失敗しない
            aligned
                .set_trailing_comment(trailing_comment)
                .expect("Setting trailing comment cannot fail because the TableFuncElement's trailing comment is already guaranteed to be on the same line as its element.");
        }

        aligned
    }
}
