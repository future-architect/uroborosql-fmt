use itertools::repeat_n;

use crate::{
    cst::{AlignedExpr, Clause, Comment, Location, UroboroSQLFmtError},
    util::convert_keyword_case,
};

use super::Expr;

/// 条件式(CASE式)を表す
#[derive(Debug, Clone)]
pub(crate) struct CondExpr {
    case_keyword: String,
    end_keyword: String,
    expr: Option<AlignedExpr>,
    when_then_clause: Vec<(Clause, Clause)>,
    else_clause: Option<Clause>,
    loc: Location,
    /// CASEキーワードの後に現れるコメント
    comments: Vec<Comment>,
}

impl CondExpr {
    pub(crate) fn new(loc: Location) -> CondExpr {
        CondExpr {
            case_keyword: "CASE".to_string(),
            end_keyword: "END".to_string(),
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

    pub(crate) fn set_case_keyword(&mut self, case_keyword: &str) {
        self.case_keyword = case_keyword.to_string();
    }

    pub(crate) fn set_end_keyword(&mut self, end_keyword: &str) {
        self.end_keyword = end_keyword.to_string();
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
            if !comment.is_block_comment() && comment.loc().is_same_line(&expr.loc()) {
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

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // depth は CASE キーワードが描画される行のインデントの深さ
        let mut result = String::new();

        // CASEキーワードの行のインデントは呼び出し側が行う
        result.push_str(&convert_keyword_case(&self.case_keyword));
        result.push('\n');

        if let Some(expr) = &self.expr {
            result.extend(repeat_n('\t', depth + 1));
            result.push_str(&expr.render(depth + 1)?);
            result.push('\n');
        }

        for comment in &self.comments {
            result.push_str(&comment.render(depth + 1)?);
            result.push('\n');
        }

        // when then
        for (when_clause, then_clause) in &self.when_then_clause {
            let formatted = when_clause.render(depth + 1)?;
            result.push_str(&formatted);

            let formatted = then_clause.render(depth + 1)?;
            result.push_str(&formatted);
        }

        // else
        if let Some(else_clause) = &self.else_clause {
            let formatted = else_clause.render(depth + 1)?;
            result.push_str(&formatted);
        }

        result.extend(repeat_n('\t', depth));
        result.push_str(&convert_keyword_case(&self.end_keyword));

        Ok(result)
    }
}
