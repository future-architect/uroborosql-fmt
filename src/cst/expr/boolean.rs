use itertools::{repeat_n, Itertools};

use crate::cst::{Comment, Location, UroboroSQLFmtError};

use super::{aligned::AlignedExpr, Expr};

// TOOD: BooleanExprをBodyでなくする
// 現状、Exprの中でBooleanExprだけがBodyになりうる
// Bodyは最初の行のインデントと最後の行の改行を自分で行う
// そのため、式をフォーマットするときに、Body(BooleanExpr)であるかをいちいち確認しなければならない。
// BooleanExprをBodyでなくして、インデントと改行は上位(SeparatedLines)で行うように変更するほうがよいと考える。
#[derive(Debug, Clone)]
pub(crate) struct BooleanExpr {
    depth: usize,              // インデントの深さ
    default_separator: String, // デフォルトセパレータ(e.g., ',', AND)
    /// separator(= AND, OR)と式、その下のコメントの組
    /// (separator, aligned, comments)
    contents: Vec<(String, AlignedExpr, Vec<Comment>)>,
    loc: Option<Location>,
    has_op: bool,
}

impl BooleanExpr {
    pub(crate) fn new(depth: usize, sep: impl Into<String>) -> BooleanExpr {
        BooleanExpr {
            depth,
            default_separator: sep.into(),
            contents: vec![] as Vec<(String, AlignedExpr, Vec<Comment>)>,
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
        if comment.is_multi_line_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
            // 行末コメントではない場合
            // 最後の要素にコメントを追加
            self.contents.last_mut().unwrap().2.push(comment);
        } else {
            // 末尾の行の行末コメントである場合
            // 最後の式にtrailing commentとして追加
            self.contents
                .last_mut()
                .unwrap()
                .1
                .set_trailing_comment(comment)?;
        }

        Ok(())
    }

    /// 左辺を展開していき、バインドパラメータをセットする
    /// 隣り合っているかどうかは、呼び出しもとで確認済みであるとする
    pub fn set_head_comment(&mut self, comment: Comment) {
        let left = &mut self.contents.first_mut().unwrap().1;
        left.set_head_comment(comment);
    }

    /// AlignedExprをセパレータ(AND/OR)とともに追加する
    fn add_aligned_expr_with_sep(&mut self, aligned: AlignedExpr, sep: String) {
        if aligned.has_rhs() {
            self.has_op = true;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        self.contents.push((sep, aligned, vec![]));
    }

    /// 式をセパレータ(AND/OR)とともに追加する
    pub(crate) fn add_expr_with_sep(&mut self, expr: Expr, sep: String) {
        // CST上ではbool式は(left op right)のような構造になっている
        // BooleanExprでは(expr1 op expr2 ... exprn)のようにフラットに保持するため、左辺がbool式ならmergeメソッドでマージする
        // また、要素をAlignedExprで保持するため、AlignedExprでない場合ラップする
        if let Expr::Boolean(boolean) = expr {
            self.merge(*boolean);
            return;
        }

        let aligned = expr.to_aligned();
        self.add_aligned_expr_with_sep(aligned, sep);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// 式を追加する
    pub(crate) fn add_expr(&mut self, expr: Expr) {
        self.add_expr_with_sep(expr, self.default_separator.clone());
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
        for (sep, aligned, _) in other.contents {
            if is_first_content {
                self.add_aligned_expr_with_sep(aligned, self.default_separator.clone());
                is_first_content = false;
            } else {
                self.add_aligned_expr_with_sep(aligned, sep);
            }
        }
    }

    /// 比較演算子で揃えたものを返す
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let align_info = self.contents.iter().map(|(_, a, _)| a).collect_vec().into();
        let mut is_first_line = true;

        for (sep, aligned, comments) in &self.contents {
            result.extend(repeat_n('\t', self.depth));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(sep);
            }
            result.push('\t');

            let formatted = aligned.render_align(self.depth, &align_info, false)?;
            result.push_str(&formatted);
            result.push('\n');

            // commentsのrender
            for comment in comments {
                result.push_str(&comment.render(self.depth)?);
                result.push('\n');
            }
        }

        Ok(result)
    }
}
