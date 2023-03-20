use itertools::repeat_n;

use crate::{
    cst::{AlignedExpr, Clause, Comment, Location, UroboroSQLFmtError},
    util::format_keyword,
};

use super::Expr;

/// 条件式(CASE式)を表す
#[derive(Debug, Clone)]
pub(crate) struct CondExpr {
    depth: usize,
    expr: Option<AlignedExpr>,
    when_then_clause: Vec<(Clause, Clause)>,
    else_clause: Option<Clause>,
    loc: Location,
    /// CASEキーワードの後に現れるコメント
    comments: Vec<Comment>,
}

impl CondExpr {
    pub(crate) fn new(loc: Location, depth: usize) -> CondExpr {
        CondExpr {
            depth,
            expr: None,
            when_then_clause: vec![],
            else_clause: None,
            loc,
            comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 単純CASE式の場合の条件式をセットする。
    pub(crate) fn set_expr(&mut self, expr: Expr) {
        self.expr = Some(expr.to_aligned())
    }

    pub(crate) fn add_when_then_clause(&mut self, when_clause: Clause, then_clause: Clause) {
        self.when_then_clause.push((when_clause, then_clause));
    }

    pub(crate) fn set_else_clause(&mut self, else_clause: Clause) {
        self.else_clause = Some(else_clause);
    }

    /// 最後の式にコメントを追加する。
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if let Some(else_clause) = self.else_clause.as_mut() {
            else_clause.add_comment_to_child(comment)?;
        } else if let Some(when_then_expr) = self.when_then_clause.last_mut() {
            when_then_expr.1.add_comment_to_child(comment)?;
        } else if let Some(expr) = self.expr.as_mut() {
            if !comment.is_multi_line_comment() && comment.loc().is_same_line(&expr.loc()) {
                expr.set_trailing_comment(comment)?;
            } else {
                self.comments.push(comment)
            }
        } else {
            // when_then/else が存在しない場合
            // つまり、CASEキーワードの直後にコメントが来た場合
            self.comments.push(comment);
        }

        Ok(())
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // CASEキーワードの行のインデントは呼び出し側が行う
        result.push_str(&format_keyword("CASE"));
        result.push('\n');

        if let Some(expr) = &self.expr {
            result.extend(repeat_n('\t', self.depth + 2));
            result.push_str(&expr.render()?);
            result.push('\n');
        }

        for comment in &self.comments {
            // when, then, elseはcaseと2つネストがずれている
            result.push_str(&comment.render(self.depth + 2)?);
            result.push('\n');
        }

        // when then
        for (when_clause, then_clause) in &self.when_then_clause {
            let formatted = when_clause.render()?;
            result.push_str(&formatted);

            let formatted = then_clause.render()?;
            result.push_str(&formatted);
        }

        // else
        if let Some(else_clause) = &self.else_clause {
            let formatted = else_clause.render()?;
            result.push_str(&formatted);
        }

        result.extend(repeat_n('\t', self.depth + 1));
        result.push_str(&format_keyword("END"));

        Ok(result)
    }
}
