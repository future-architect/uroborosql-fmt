use itertools::Itertools;

use crate::{
    cst::{add_indent, AlignInfo, AlignedExpr, Comment, Location, ParenthesizedExprList},
    error::UroboroSQLFmtError,
    util::{add_space_by_range, count_width, tab_size, trim_bind_param},
};

/// 列のリストを表す。
#[derive(Debug, Clone)]
pub(crate) struct ColumnList {
    cols: Vec<AlignedExpr>,
    loc: Location,
    /// 複数行で出力するかを指定するフラグ。
    /// デフォルトでは false (つまり、単一行で出力する) になっている。
    force_multi_line: bool,
    /// バインドパラメータ
    head_comment: Option<String>,
    /// 開き括弧と最初の式との間のコメント
    start_comments: Vec<Comment>,
}

impl ColumnList {
    pub(crate) fn new(
        cols: Vec<AlignedExpr>,
        loc: Location,
        start_comments: Vec<Comment>,
    ) -> ColumnList {
        ColumnList {
            cols,
            loc,
            force_multi_line: false,
            head_comment: None,
            start_comments,
        }
    }

    pub(crate) fn from_expr_list(
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

        Ok(ColumnList::new(exprs, location, start_comments))
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn force_multi_line(&self) -> bool {
        self.force_multi_line
    }

    pub(crate) fn last_line_len(&self, acc: usize) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let mut current_len = acc + "(".len();
            if let Some(param) = &self.head_comment {
                current_len += count_width(param)
            };

            self.cols.iter().enumerate().for_each(|(i, col)| {
                current_len += col.last_line_len_from_left(current_len);
                if i != self.cols.len() - 1 {
                    current_len += ", ".len()
                }
            });
            current_len + ")".len()
        }
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment { text, mut loc } = comment;

        let text = trim_bind_param(text);

        self.head_comment = Some(text);
        loc.append(self.loc());
        self.loc = loc;
    }

    /// 列リストを複数行で描画するかを指定する。
    /// true を与えたら必ず複数行で描画され、false を与えたらできるだけ単一行で描画する。
    pub(crate) fn set_force_multi_line(&mut self, b: bool) {
        self.force_multi_line = b
    }

    /// 複数行で描画するかどうかを bool 型の値で取得する。
    /// 複数行で描画する場合は true を返す。
    /// 自身の is_multi_line のオプションの値だけでなく、開き括弧と最初の式との間にコメントを持つどうか、各列が単一行かどうか、各行が末尾コメントを持つかどうかも考慮する。
    pub(crate) fn is_multi_line(&self) -> bool {
        self.force_multi_line
            || !self.start_comments.is_empty()
            || self
                .cols
                .iter()
                .any(|a| a.is_multi_line() || a.has_trailing_comment())
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

        if self.is_multi_line() {
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
        } else {
            // ColumnListを単一行で描画する
            result.push('(');
            result.push_str(
                &self
                    .cols
                    .iter()
                    .map(|e| e.render(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", "),
            );
            result.push(')');
        }

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}

impl TryFrom<ParenthesizedExprList> for ColumnList {
    type Error = UroboroSQLFmtError;

    fn try_from(paren_list: ParenthesizedExprList) -> Result<Self, Self::Error> {
        // いずれかの ExprListItem に following_comments がある場合はエラーにする
        let mut exprs = Vec::new();
        for item in paren_list.expr_list.items() {
            if let Some(following_comment) = item.following_comments().first() {
                return Err(UroboroSQLFmtError::Unimplemented(format!(
                    "Comments following columns are not supported. Only trailing comments are supported.\ncomment: {}",
                    following_comment.text()
                )));
            }
            exprs.push(item.expr().clone());
        }

        Ok(ColumnList::new(
            exprs,
            paren_list.location,
            paren_list.start_comments,
        ))
    }
}
