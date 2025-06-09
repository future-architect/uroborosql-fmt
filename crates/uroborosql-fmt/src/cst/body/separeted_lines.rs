use itertools::Itertools;

use crate::{
    cst::{add_indent, AlignInfo, AlignedExpr, Comment, Expr, Location},
    error::UroboroSQLFmtError,
    util::{add_single_space, add_space_by_range, tab_size, to_tab_num},
};

#[derive(Debug, Clone)]
pub(crate) struct SepLinesContent {
    sep: Option<String>,
    /// sep と expr の間に現れるコメント
    preceding_comments: Vec<Comment>,
    expr: AlignedExpr,
    following_comments: Vec<Comment>,
}

impl From<Expr> for SeparatedLines {
    /// 一つのExprからなるBodyを生成し返す
    fn from(expr: Expr) -> SeparatedLines {
        if expr.is_body() {
            // BooleanはSeparatedLinesで表現されるので、そのSeparatedLinesをBodyとして返す
            if let Expr::Boolean(boolean) = expr {
                *boolean
            } else {
                // 現状Expr::Boolean()以外にBodyとなりうるExprは存在しないので到達しない
                unreachable!()
            }
        } else {
            // Bodyでない場合、SeparatedLinesにして返す
            let mut sep_lines = SeparatedLines::new();
            sep_lines.add_expr(expr.to_aligned(), None, vec![]);
            sep_lines
        }
    }
}

impl SepLinesContent {
    fn new(
        sep: Option<String>,
        expr: AlignedExpr,
        preceding_comments: Vec<Comment>,
        following_comments: Vec<Comment>,
    ) -> SepLinesContent {
        SepLinesContent {
            sep,
            preceding_comments,
            expr,
            following_comments,
        }
    }

    fn get_aligned(&self) -> &AlignedExpr {
        &self.expr
    }

    fn get_aligned_mut(&mut self) -> &mut AlignedExpr {
        &mut self.expr
    }

    fn add_following_comments(&mut self, comment: Comment) {
        self.following_comments.push(comment)
    }

    fn set_trailing_comment(&mut self, comment: Comment) -> Result<(), UroboroSQLFmtError> {
        self.expr.set_trailing_comment(comment)
    }

    fn set_head_comment(&mut self, comment: Comment) {
        self.expr.set_head_comment(comment)
    }

    fn sep_len(&self) -> usize {
        self.sep.as_ref().map_or(0, |sep| sep.len())
    }

    fn render(
        &self,
        align_info: &AlignInfo,
        max_sep_len: usize,
        depth: usize,
    ) -> Result<String, UroboroSQLFmtError> {
        if depth < 1 {
            // 'AND'\'OR'の後にタブ文字を挿入するので、インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::Rendering(
                "SepLinesContent::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();
        add_indent(&mut result, depth - 1);

        // separatorがある(=最初の行でない)場合はseparatorを描画
        if let Some(sep) = &self.sep {
            result.push_str(sep);
        }

        // sepを考慮したdepth
        let new_depth_with_sep = depth - 1 + to_tab_num(1.max(max_sep_len));

        // セパレータ (カンマ, AND, OR) と式の間に現れるコメント
        if !self.preceding_comments.is_empty() {
            let mut is_first = true;
            for comment in &self.preceding_comments {
                if comment.is_two_way_sql_comment() {
                    if is_first {
                        is_first = false;

                        result.push('\n');
                    }
                    result.push_str(&comment.render(depth - 1)?);
                } else if is_first {
                    is_first = false;
                    add_single_space(&mut result);
                    result.push_str(&comment.render(0)?);
                } else {
                    result.push_str(&comment.render(new_depth_with_sep)?);
                }

                result.push('\n');
            }

            if !self.expr.is_lhs_cond() {
                // コメントの挿入後に改行をしたので、タブを挿入
                add_indent(&mut result, new_depth_with_sep);
            } else {
                // 左辺がCASE文の場合は挿入した改行を削除
                result.pop();
            }
        } else {
            // コメントが存在しない場合はseparatorの直後にタブを挿入
            let start_col = (depth - 1) * tab_size() + self.sep_len();
            let end_col = new_depth_with_sep * tab_size();
            add_space_by_range(&mut result, start_col, end_col);
        }

        let formatted = self.expr.render_align(new_depth_with_sep, align_info)?;
        result.push_str(&formatted);
        result.push('\n');

        // commentsのrender
        for comment in &self.following_comments {
            result.push_str(&comment.render(depth - 1)?);
            result.push('\n');
        }

        Ok(result)
    }
}

/// あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub(crate) struct SeparatedLines {
    contents: Vec<SepLinesContent>,
    loc: Option<Location>,
}

impl SeparatedLines {
    pub(crate) fn new() -> SeparatedLines {
        SeparatedLines {
            contents: vec![],
            loc: None,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    /// 式をセパレータ(AND/OR)とともに追加する
    pub(crate) fn add_expr(
        &mut self,
        aligned: AlignedExpr,
        sep: Option<String>,
        preceding_comments: Vec<Comment>,
    ) {
        // CST上ではbool式は(left op right)のような構造になっている
        // BooleanExprでは(expr1 op expr2 ... exprn)のようにフラットに保持するため、左辺がbool式ならmergeメソッドでマージする
        // また、要素をAlignedExprで保持するため、AlignedExprでない場合ラップする

        self.add_content(aligned, sep, preceding_comments, vec![]);
    }

    /// boolean_expr同士をマージする
    ///
    /// # Example
    /// ### boolean1
    /// ```sql
    ///     a
    /// AND b
    /// AND c
    /// ```
    ///
    /// ### boolean2
    /// ```sql
    ///     x
    /// AND z
    /// ```
    ///
    /// boolean1にboolean2をmerge_sep="OR"でマージすると以下のようになる
    /// ### boolean1
    /// ```sql
    ///     a
    /// AND b
    /// AND c
    /// OR  x
    /// AND z
    /// ```
    pub(crate) fn merge_boolean_expr(&mut self, merge_sep: String, other: SeparatedLines) {
        for content in &other.contents {
            let SepLinesContent {
                sep,
                expr: aligned,
                preceding_comments: preceding,
                following_comments: following,
            } = content.clone();

            if sep.is_none() {
                // separatorがない = 1行目
                // マージ対象の1行目はmerge_sepをseparatorとする
                self.add_content(aligned, Some(merge_sep.clone()), preceding, following);
            } else {
                self.add_content(aligned, sep, preceding, following);
            }
        }
    }

    /// 左辺を展開していき、バインドパラメータをセットする
    /// 隣り合っているかどうかは、呼び出しもとで確認済みであるとする
    pub fn set_head_comment(&mut self, comment: Comment) {
        self.contents.first_mut().unwrap().set_head_comment(comment)
    }

    /// SeparatedLineContent を生成し、自身の contents に追加する。
    fn add_content(
        &mut self,
        mut aligned: AlignedExpr,
        sep: Option<String>,
        mut preceding_comments: Vec<Comment>,
        following_comments: Vec<Comment>,
    ) {
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        if let Some(last_preceding) = preceding_comments.last() {
            // preceding_commentの最後が続く式に隣り合っている場合、それをhead_commentに移動する
            if last_preceding.loc().is_next_to(&aligned.loc()) {
                aligned.set_head_comment(last_preceding.clone());
                preceding_comments.pop();
            }
        }

        self.contents.push(SepLinesContent::new(
            sep,
            aligned,
            preceding_comments,
            following_comments,
        ))
    }

    /// 最後の式にコメントを追加する
    /// 最後の式と同じ行である場合は行末コメントとして追加し、そうでない場合は式の下のコメントとして追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if let Some(last_content) = self.contents.last_mut() {
            if comment.is_block_comment() || !last_content.expr.loc().is_same_line(&comment_loc) {
                // 行末コメントではない場合 (ブロックコメント or 最後のexprのコメントが同一行でない)
                // ここで、self.loc()ではなく最後のexprのlocと比較している理由は、もしself.loc()と比較をしているとコメントの入れ替わりが発生する可能性があるため。
                // 例えば、以下のようなSQLでself.loc()を用いて比較を行うと、TBLの後ろにsingle_ling_commentが付いてしまう
                // ```
                // SELECT *
                // FROM TBL
                // /*block_commnet*/ -- single_line_comment
                // ```

                // 最後の要素にコメントを追加
                last_content.add_following_comments(comment);
            } else {
                // 末尾の行の行末コメントである場合
                // 最後の式にtrailing commentとして追加
                last_content.set_trailing_comment(comment)?;
            }
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(comment_loc),
            None => self.loc = Some(comment_loc),
        };

        Ok(())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    pub(crate) fn try_set_head_comment(&mut self, comment: Comment) -> bool {
        if let Some(first_content) = self.contents.first_mut() {
            let first_aligned: &mut AlignedExpr = first_content.get_aligned_mut();
            if comment.loc().is_next_to(&first_aligned.loc()) {
                first_aligned.set_head_comment(comment);
                return true;
            }
        }
        false
    }

    /// 最初の要素にセパレータを追加する
    pub(crate) fn set_first_separator(&mut self, sep: String) {
        if let Some(first_content) = self.contents.first_mut() {
            first_content.sep = Some(sep);
        }
    }

    /// separatorで揃えたものを返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // Vec<AlignedExpr>からAlignInfoを作成
        let align_info = self
            .contents
            .iter()
            .map(|c| c.get_aligned())
            .collect_vec()
            .into();

        // sepの最大長を取得
        let max_sep_len = self
            .contents
            .iter()
            .map(|c| c.sep_len())
            .max()
            .unwrap_or_default();

        // 各コンテンツをAlignInfoを用いて描画
        for content in &self.contents {
            result.push_str(&content.render(&align_info, max_sep_len, depth)?);
        }

        Ok(result)
    }
}
