use itertools::Itertools;

use crate::{
    cst::{add_indent, AlignInfo, AlignedExpr, Comment, Expr, Location},
    error::UroboroSQLFmtError,
    util::{add_space_by_range, count_width, tab_size, trim_bind_param},
};

#[derive(Debug, Clone)]
pub(crate) struct SpaceSeparatedColumnExpr {
    left: Expr,
    sep: Option<String>,
    right: Option<Expr>,
}

impl SpaceSeparatedColumnExpr {
    pub(crate) fn new(left: Expr, sep: Option<String>, right: Option<Expr>) -> Self {
        Self { left, sep, right }
    }

    pub(crate) fn last_line_len(&self) -> usize {
        let mut len = self.left.last_line_len_from_left(0);

        if let Some(sep) = &self.sep {
            len += " ".len() + sep.len();
        }

        if let Some(right) = &self.right {
            len += " ".len() + right.last_line_len_from_left(0);
        }

        len
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.left.render(0)?);

        if let Some(sep) = &self.sep {
            result.push(' ');
            result.push_str(sep);
        }
        if let Some(right) = &self.right {
            result.push(' ');
            result.push_str(&right.render(0)?);
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SpaceSeparatedColumnList {
    cols: Vec<SpaceSeparatedColumnExpr>,
    loc: Location,
    head_comment: Option<String>,
}

impl SpaceSeparatedColumnList {
    pub(crate) fn new(cols: Vec<SpaceSeparatedColumnExpr>, loc: Location) -> Self {
        Self {
            cols,
            loc,
            head_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_len(&self, acc: usize) -> usize {
        let mut current_len = acc + "(".len();
        if let Some(param) = &self.head_comment {
            current_len += count_width(param)
        };

        self.cols.iter().enumerate().for_each(|(i, col)| {
            current_len += col.last_line_len();
            if i != self.cols.len() - 1 {
                current_len += ", ".len()
            }
        });
        current_len + ")".len()
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        self.head_comment = Some(comment.text().to_string());
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        if let Some(head_comment) = &self.head_comment {
            result.push_str(head_comment);
        }

        result.push('(');

        let rendered_cols = self
            .cols
            .iter()
            .map(|col| col.render())
            .collect::<Result<Vec<String>, UroboroSQLFmtError>>()?
            .join(", ");
        result.push_str(&rendered_cols);

        result.push(')');

        Ok(result)
    }
}

impl From<SpaceSeparatedColumnList> for MultiLineColumnList {
    fn from(list: SpaceSeparatedColumnList) -> Self {
        let aligned_exprs = list
            .cols
            .iter()
            .map(|col| AlignedExpr::from(col.clone()))
            .collect::<Vec<_>>();

        MultiLineColumnList::new(aligned_exprs, list.loc.clone(), vec![])
    }
}

impl From<SpaceSeparatedColumnExpr> for AlignedExpr {
    fn from(col: SpaceSeparatedColumnExpr) -> Self {
        let mut aligned = AlignedExpr::new(col.left);

        if let Some(right) = col.right {
            aligned.add_rhs(col.sep, right);
        }

        aligned
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MultiLineColumnList {
    cols: Vec<AlignedExpr>,
    loc: Location,
    head_comment: Option<String>,
    start_comments: Vec<Comment>,
}

impl MultiLineColumnList {
    pub(crate) fn new(cols: Vec<AlignedExpr>, loc: Location, start_comments: Vec<Comment>) -> Self {
        Self {
            cols,
            loc,
            head_comment: None,
            start_comments,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_len(&self) -> usize {
        ")".len()
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment { text, mut loc } = comment;

        let text = trim_bind_param(text);

        self.head_comment = Some(text);
        loc.append(self.loc());
        self.loc = loc;
    }

    /// カラムリストをrenderする。
    /// 自身の is_multi_line() が true になる場合には複数行で描画し、false になる場合単一行で描画する。
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は開きかっこを描画する行のインデントの深さ
        let mut result = String::new();

        // バインドパラメータがある場合、最初に描画
        if let Some(bind_param) = &self.head_comment {
            result.push_str(bind_param);
        }

        // 各列を複数行に出力する

        result.push_str("(\n");

        // 開き括弧の後のコメント
        for comment in &self.start_comments {
            result.push_str(&comment.render(depth + 1)?);
            result.push('\n');
        }

        // 最初の行のインデント
        add_indent(&mut result, depth + 1);

        // 各要素間の改行、カンマ、インデント
        let mut separator = "\n".to_string();
        add_indent(&mut separator, depth);
        separator.push(',');
        add_space_by_range(&mut separator, 1, tab_size());

        // Vec<AlignedExpr> -> Vec<&AlignedExpr>
        let aligned_exprs = self.cols.iter().collect_vec();
        let align_info = AlignInfo::from(aligned_exprs);

        result.push_str(
            &self
                .cols
                .iter()
                .map(|a| a.render_align(depth + 1, &align_info))
                .collect::<Result<Vec<_>, _>>()?
                .join(&separator),
        );

        result.push('\n');
        add_indent(&mut result, depth);
        result.push(')');

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}

/// 列のリストを表す。
#[derive(Debug, Clone)]
pub(crate) enum ColumnList {
    SingleLine(SpaceSeparatedColumnList),
    MultiLine(MultiLineColumnList),
}

impl ColumnList {
    pub(crate) fn new(cols: Vec<AlignedExpr>, loc: Location, start_comments: Vec<Comment>) -> Self {
        // 以下のいずれかに該当したら MultiLine で描画する
        // - start_comments がある
        // - いずれかのAlignedExpr に行末コメントがある
        // - いずれかのAlignedExpr が複数行で描画される
        let is_multi_line = !start_comments.is_empty()
            || cols
                .iter()
                .any(|a| a.is_multi_line() || a.has_trailing_comment());

        if is_multi_line {
            Self::MultiLine(MultiLineColumnList::new(cols, loc, start_comments))
        } else {
            // AlignedExpr を SingleLineColumn に変換
            let single_line_cols = cols
                .iter()
                .map(|aligned| SpaceSeparatedColumnExpr::from(aligned.clone()))
                .collect();

            Self::SingleLine(SpaceSeparatedColumnList::new(single_line_cols, loc))
        }
    }

    pub(crate) fn try_from_expr_list(
        expr_list: &crate::cst::ExprList,
        location: crate::cst::Location,
        start_comments: Vec<Comment>,
    ) -> Result<Self, crate::error::UroboroSQLFmtError> {
        // いずれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for item in expr_list.items() {
            if let Some(following_comment) = item.following_comments().first() {
                return Err(crate::error::UroboroSQLFmtError::Unimplemented(
                    format!(
                        "Comments following columns are not supported. Only trailing comments are supported.\ncomment: {}",
                        following_comment.text()
                    ),
                ));
            }

            exprs.push(item.expr().clone());
        }

        Ok(Self::new(exprs, location, start_comments))
    }

    /// 複数行で描画するかどうかを bool 型の値で返す
    pub(crate) fn is_multi_line(&self) -> bool {
        matches!(self, Self::MultiLine(_))
    }

    pub(crate) fn force_multi_line(&mut self) {
        if let Self::SingleLine(single_line_list) = self {
            *self = Self::MultiLine(single_line_list.clone().into());
        }
    }

    pub(crate) fn loc(&self) -> Location {
        match self {
            Self::SingleLine(list) => list.loc(),
            Self::MultiLine(list) => list.loc(),
        }
    }

    pub(crate) fn last_line_len(&self, acc: usize) -> usize {
        match self {
            Self::SingleLine(list) => list.last_line_len(acc),
            Self::MultiLine(list) => list.last_line_len(),
        }
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        match self {
            Self::SingleLine(list) => list.set_head_comment(comment),
            Self::MultiLine(list) => list.set_head_comment(comment),
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            Self::SingleLine(list) => list.render(),
            Self::MultiLine(list) => list.render(depth),
        }
    }
}
