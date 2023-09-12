use crate::error::UroboroSQLFmtError;

use super::{Clause, Comment, Location};

// *_statementに対応した構造体
#[derive(Debug, Clone)]
pub(crate) struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Location>,
    /// Statementの上に現れるコメント
    comments: Vec<Comment>,
    /// 末尾にセミコロンがついているか
    has_semi: bool,
}

impl Statement {
    pub(crate) fn new() -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
            comments: vec![] as Vec<Comment>,
            has_semi: false,
        }
    }

    /// ClauseのVecへの参照を取得する
    pub(crate) fn get_clauses(self) -> Vec<Clause> {
        self.clauses
    }

    // 文に句を追加する
    pub(crate) fn add_clause(&mut self, clause: Clause) {
        match &mut self.loc {
            Some(loc) => loc.append(clause.loc()),
            None => self.loc = Some(clause.loc()),
        }
        self.clauses.push(clause);
    }

    // 文に複数の句を追加する
    pub(crate) fn add_clauses(&mut self, clauses: Vec<Clause>) {
        for clause in clauses {
            self.add_clause(clause);
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        self.clauses
            .last_mut()
            .unwrap()
            .add_comment_to_child(comment)?;

        Ok(())
    }

    // Statementの上に現れるコメントを追加する
    pub(crate) fn add_comment(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    /// 末尾にセミコロンがつくかどうかを指定する
    pub(crate) fn set_semi(&mut self, has_semi: bool) {
        self.has_semi = has_semi;
    }

    pub(crate) fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        // clause1
        // ...
        // clausen
        let mut result = String::new();

        for comment in &self.comments {
            result.push_str(&comment.render(depth)?);
            result.push('\n');
        }

        // 1つでもエラーの場合は全体もエラー
        for clause in &self.clauses {
            result.push_str(&clause.render(depth)?);
        }

        if self.has_semi {
            result.push_str(";\n");
        }

        Ok(result)
    }
}
