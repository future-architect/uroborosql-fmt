use itertools::{repeat_n, Itertools};

use crate::{
    cst::{AlignInfo, Comment, Location, UroboroSQLFmtError},
    util::convert_keyword_case,
};

use super::{aligned::AlignedExpr, Expr};

/// 演算子(セパレータ)、式、演算子と式の間のコメント、式の直後に来るコメントの4つ組を表す。
/// BooleanExpr における1要素として使用する。
#[derive(Debug, Clone)]
struct BooleanExprContent {
    op: String,
    /// op と expr の間に現れるコメント
    preceding_comments: Vec<Comment>,
    expr: AlignedExpr,
    following_comments: Vec<Comment>,
}

impl BooleanExprContent {
    fn new(
        op: impl Into<String>,
        expr: AlignedExpr,
        preceding_comments: Vec<Comment>,
        following_comments: Vec<Comment>,
    ) -> BooleanExprContent {
        BooleanExprContent {
            op: op.into(),
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

    fn to_tuple(&self) -> (String, AlignedExpr, Vec<Comment>, Vec<Comment>) {
        (
            self.op.clone(),
            self.expr.clone(),
            self.preceding_comments.clone(),
            self.following_comments.clone(),
        )
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

    /// is_first_lineは、BooleanExpr の最初の Content であるかどうかを bool 値で与える。
    fn render(
        &self,
        align_info: &AlignInfo,
        depth: usize,
        is_first_line: bool,
    ) -> Result<String, UroboroSQLFmtError> {
        if depth < 1 {
            // 'AND'\'OR'の後にタブ文字を挿入するので、インデントの深さ(depth)は1以上でなければならない。
            return Err(UroboroSQLFmtError::Rendering(
                "BooleanExprContent::render(): The depth must be bigger than 0".to_owned(),
            ));
        }

        let mut result = String::new();
        result.extend(repeat_n('\t', depth - 1));

        // 最初の行でなければ演算子を挿入
        if !is_first_line {
            result.push_str(&convert_keyword_case(&self.op));
        }

        result.push('\t');

        // AND/OR と式の間に現れるコメント
        if !self.preceding_comments.is_empty() {
            let mut is_first = true;
            for comment in &self.preceding_comments {
                if is_first {
                    is_first = false;
                    result.push_str(&comment.render(0)?);
                } else {
                    result.push_str(&comment.render(depth)?);
                }
                result.push('\n');
            }

            if self.expr.is_lhs_cond() {
                // CASE文である場合この後の処理で改行を挿入するため、ここでは最後の改行を削除する
                result.pop();
            } else {
                // コメントの挿入後に改行をしたので、タブを挿入
                result.extend(repeat_n('\t', depth));
            }
        } else if self.expr.is_lhs_cond() {
            // コメントがない場合、現在のresultの末尾はop\tとなっている
            // CASE文の場合はopの直後に改行を行うため、演算子の直後のタブを削除
            result.pop();
        }

        // CASE文である場合、改行してインデントを挿入
        if self.expr.is_lhs_cond() {
            result.push('\n');
            result.extend(repeat_n('\t', depth));
        }

        let formatted = self.expr.render_align(depth, align_info, false)?;
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

// TOOD: BooleanExprをBodyでなくする
// 現状、Exprの中でBooleanExprだけがBodyになりうる
// Bodyは最初の行のインデントと最後の行の改行を自分で行う
// そのため、式をフォーマットするときに、Body(BooleanExpr)であるかをいちいち確認しなければならない。
// BooleanExprをBodyでなくして、インデントと改行は上位(SeparatedLines)で行うように変更するほうがよいと考える。
#[derive(Debug, Clone)]
pub(crate) struct BooleanExpr {
    default_separator: String, // デフォルトセパレータ(e.g., ',', AND)
    /// separator(= AND, OR)と式、その下のコメントの組
    /// (separator, aligned, comments)
    contents: Vec<BooleanExprContent>,
    loc: Option<Location>,
    has_op: bool,
}

impl BooleanExpr {
    pub(crate) fn new(sep: impl Into<String>) -> BooleanExpr {
        BooleanExpr {
            default_separator: sep.into(),
            contents: vec![] as Vec<BooleanExprContent>,
            loc: None,
            has_op: false,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    pub(crate) fn set_default_separator(&mut self, sep: impl Into<String>) {
        self.default_separator = sep.into();
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_block_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            // 最後の要素にコメントを追加
            self.contents
                .last_mut()
                .unwrap()
                .add_following_comments(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.contents
                .last_mut()
                .unwrap()
                .set_trailing_comment(comment)?;
        }

        Ok(())
    }

    /// 左辺を展開していき、バインドパラメータをセットする
    /// 隣り合っているかどうかは、呼び出しもとで確認済みであるとする
    pub fn set_head_comment(&mut self, comment: Comment) {
        self.contents.first_mut().unwrap().set_head_comment(comment)
    }

    /// BooleanExprContent を生成し、自身の contents に追加する。
    fn add_content(
        &mut self,
        mut aligned: AlignedExpr,
        sep: String,
        mut preceding_comments: Vec<Comment>,
        following_comments: Vec<Comment>,
    ) {
        if aligned.has_rhs() {
            self.has_op = true;
        }

        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        if let Some(last_preceding) = preceding_comments.last() {
            if last_preceding.loc().is_next_to(&aligned.loc()) {
                aligned.set_head_comment(last_preceding.clone());
                preceding_comments.pop();
            }
        }

        self.contents.push(BooleanExprContent::new(
            sep,
            aligned,
            preceding_comments,
            following_comments,
        ))
    }

    /// 式をセパレータ(AND/OR)とともに追加する
    fn add_expr_with_sep(&mut self, expr: Expr, sep: String, preceding_comments: Vec<Comment>) {
        // CST上ではbool式は(left op right)のような構造になっている
        // BooleanExprでは(expr1 op expr2 ... exprn)のようにフラットに保持するため、左辺がbool式ならmergeメソッドでマージする
        // また、要素をAlignedExprで保持するため、AlignedExprでない場合ラップする
        if let Expr::Boolean(boolean) = expr {
            self.merge(*boolean);
            return;
        }

        let aligned = expr.to_aligned();
        self.add_content(aligned, sep, preceding_comments, vec![]);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// 式を追加する
    pub(crate) fn add_expr(&mut self, expr: Expr) {
        self.add_expr_with_sep(expr, self.default_separator.clone(), vec![]);
    }

    /// 演算子と式の間に現れるコメント(preceding_comment)と式を追加する。
    pub(crate) fn add_expr_with_preceding_comments(&mut self, expr: Expr, preceding: Vec<Comment>) {
        self.add_expr_with_sep(expr, self.default_separator.clone(), preceding)
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

    /// BooleanExprとBooleanExprをマージする
    pub(crate) fn merge(&mut self, other: BooleanExpr) {
        // そろえる演算子があるか
        self.has_op = self.has_op || other.has_op;

        // separatorをマージする
        //
        // ["AND", "AND"]
        // ["OR", "OR", "OR"]
        // default_separator = "DEF"
        //
        // => ["AND", "AND", "DEF", "OR", "OR"]

        let mut is_first_content = true;
        for content in &other.contents {
            let (sep, aligned, preceding, following) = content.to_tuple();
            if is_first_content {
                self.add_content(
                    aligned,
                    self.default_separator.clone(),
                    preceding,
                    following,
                );
                is_first_content = false;
            } else {
                self.add_content(aligned, sep, preceding, following);
            }
        }
    }

    /// 比較演算子で揃えたものを返す
    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let align_info = self
            .contents
            .iter()
            .map(|c| c.get_aligned())
            .collect_vec()
            .into();
        let mut is_first_line = true;

        for content in &self.contents {
            result.push_str(&content.render(&align_info, depth, is_first_line)?);
            if is_first_line {
                is_first_line = false;
            }
        }

        Ok(result)
    }
}
