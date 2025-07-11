use crate::cst::{
    add_indent, joined_table::JoinedTable, AlignInfo, AlignedExpr, Comment, Location,
};
use crate::error::UroboroSQLFmtError;
use crate::util::single_space;
use crate::visitor::COMMA;

#[derive(Debug, Clone)]
pub(crate) enum TableRef {
    // AlignedExprで表現可能なテーブル参照（括弧付きJOINも含む）
    SimpleTable(AlignedExpr),
    // 括弧なしのJOIN構造
    JoinedTable(Box<JoinedTable>),
}

impl TableRef {
    pub(crate) fn loc(&self) -> Location {
        match self {
            TableRef::SimpleTable(aligned_expr) => aligned_expr.loc(),
            TableRef::JoinedTable(joined_table) => joined_table.loc(),
        }
    }

    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            TableRef::SimpleTable(aligned_expr) => aligned_expr.set_trailing_comment(comment),
            TableRef::JoinedTable(joined_table) => joined_table.add_comment_to_child(comment),
        }
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        match self {
            TableRef::SimpleTable(aligned_expr) => aligned_expr.set_head_comment(comment),
            TableRef::JoinedTable(joined_table) => {
                joined_table.set_head_comment(comment);
            }
        }
    }

    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        match self {
            TableRef::SimpleTable(aligned_expr) => aligned_expr.last_line_len_from_left(acc),
            TableRef::JoinedTable(joined_table) => joined_table.last_line_len_from_left(acc),
        }
    }

    /// 縦揃え可能なAlignedExprを取得する
    pub(crate) fn get_alignable_expr(&self) -> Option<&AlignedExpr> {
        match self {
            TableRef::SimpleTable(aligned_expr) => Some(aligned_expr),
            TableRef::JoinedTable(_) => None, // JoinedTableは縦揃え対象外
        }
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        match self {
            TableRef::SimpleTable(aligned_expr) => aligned_expr.render(depth),
            TableRef::JoinedTable(joined_table) => joined_table.render(depth),
        }
    }

    /// 縦揃え情報を考慮してrenderする
    pub(crate) fn render_with_align(
        &self,
        depth: usize,
        align_info: Option<&AlignInfo>,
    ) -> Result<String, UroboroSQLFmtError> {
        match self {
            TableRef::SimpleTable(aligned_expr) => {
                if let Some(align_info) = align_info {
                    aligned_expr.render_align(depth, align_info)
                } else {
                    aligned_expr.render(depth)
                }
            }
            TableRef::JoinedTable(joined_table) => {
                // JoinedTableは縦揃え対象外なので通常のrender
                joined_table.render(depth)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FromList {
    contents: Vec<TableRef>,
    loc: Option<Location>,
    extra_leading_comma: Option<String>,
    following_comments: Vec<Comment>,
}

impl FromList {
    pub(crate) fn new() -> FromList {
        FromList {
            contents: vec![],
            loc: None,
            extra_leading_comma: None,
            following_comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    pub(crate) fn set_extra_leading_comma(&mut self, comma: Option<String>) {
        self.extra_leading_comma = comma;
    }

    pub(crate) fn add_table_ref(&mut self, table_ref: TableRef) {
        match &mut self.loc {
            Some(loc) => loc.append(table_ref.loc()),
            None => self.loc = Some(table_ref.loc()),
        }

        self.contents.push(table_ref);
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if let Some(last_table_ref) = self.contents.last_mut() {
            if comment.is_block_comment() || !last_table_ref.loc().is_same_line(&comment_loc) {
                self.add_following_comments(comment);
            } else {
                // 末尾の行の行末コメントである場合
                // 最後のTableRefにtrailing commentとして追加
                last_table_ref.set_trailing_comment(comment)?;
            }
        }

        match &mut self.loc {
            Some(loc) => loc.append(comment_loc),
            None => self.loc = Some(comment_loc),
        };

        Ok(())
    }

    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        if let Some(table_ref) = self.contents.first_mut() {
            if comment.loc().is_next_to(&table_ref.loc()) {
                table_ref.set_head_comment(comment);
                return true;
            }
        }

        false
    }

    fn add_following_comments(&mut self, comment: Comment) {
        self.following_comments.push(comment);
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // 縦揃え可能な要素からAlignInfoを作成
        let alignable_exprs: Vec<&AlignedExpr> = self
            .contents
            .iter()
            .filter_map(|table_ref| table_ref.get_alignable_expr())
            .collect();

        let align_info = if !alignable_exprs.is_empty() {
            Some(AlignInfo::from(alignable_exprs))
        } else {
            None
        };

        let mut result = String::new();

        let Some((first, rest)) = self.contents.split_first() else {
            return Err(UroboroSQLFmtError::IllegalOperation(
                "FromList is empty".to_string(),
            ));
        };

        // 先頭要素の前にカンマがある場合
        if let Some(ref comma) = self.extra_leading_comma {
            result.push_str(comma);
        }

        // 先頭要素の render
        add_indent(&mut result, depth);
        result.push_str(&first.render_with_align(depth, align_info.as_ref())?);

        // 残りの要素を render
        for table_ref in rest {
            result.push('\n');

            add_indent(&mut result, depth - 1);
            result.push_str(COMMA);
            result.push(single_space());

            result.push_str(&table_ref.render_with_align(depth, align_info.as_ref())?);
        }

        for comment in &self.following_comments {
            result.push('\n');
            result.push_str(&comment.render(depth - 1)?);
        }

        result.push('\n');

        Ok(result)
    }
}
