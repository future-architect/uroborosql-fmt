use itertools::{repeat_n, Itertools};

use crate::{
    cst::{AlignedExpr, Comment, Location},
    error::UroboroSQLFmtError,
};

/// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub(crate) struct SeparatedLines {
    /// セパレータ(e.g., ',', AND)
    separator: String,
    /// 各行の情報。式と直後に来るコメントのペアのベクトルとして保持する
    contents: Vec<(AlignedExpr, Vec<Comment>)>,
    loc: Option<Location>,
    /// 縦ぞろえの対象となる演算子があるかどうか
    has_op: bool,
}

impl SeparatedLines {
    pub(crate) fn new(sep: impl Into<String>) -> SeparatedLines {
        let separator = sep.into();
        SeparatedLines {
            separator,
            contents: vec![] as Vec<(AlignedExpr, Vec<Comment>)>,
            loc: None,
            has_op: false,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // 式を追加する
    pub(crate) fn add_expr(&mut self, aligned: AlignedExpr) {
        // 演算子があるかどうかをチェック
        if aligned.has_rhs() {
            self.has_op = true;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        self.contents.push((aligned, vec![]));
    }

    /// 最後の式にコメントを追加する
    /// 最後の式と同じ行である場合は行末コメントとして追加し、そうでない場合は式の下のコメントとして追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        let comment_loc = comment.loc();

        if comment.is_block_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            // 最後の要素にコメントを追加
            self.contents.last_mut().unwrap().1.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.contents
                .last_mut()
                .unwrap()
                .0
                .set_trailing_comment(comment)?;
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
        if let Some((first_aligned, _)) = self.contents.first_mut() {
            if comment.loc().is_next_to(&first_aligned.loc()) {
                first_aligned.set_head_comment(comment);
                return true;
            }
        }
        false
    }

    /// AS句で揃えたものを返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        if depth < 1 {
            // ','の後にタブ文字を挿入するので、インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::Rendering(
                "SeparatedLines::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();

        // 演算子自体の長さ
        let align_info = self.contents.iter().map(|(a, _)| a).collect_vec().into();
        let mut is_first_line = true;

        for (aligned, comments) in &self.contents {
            result.extend(repeat_n('\t', depth - 1));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(&self.separator);
            }
            result.push('\t');

            // alignedに演算子までの最長の長さを与えてフォーマット済みの文字列をもらう
            let formatted = aligned.render_align(depth, &align_info)?;
            result.push_str(&formatted);
            result.push('\n');

            // commentsのrender
            for comment in comments {
                result.push_str(&comment.render(depth - 1)?);
                result.push('\n');
            }
        }

        Ok(result)
    }
}
