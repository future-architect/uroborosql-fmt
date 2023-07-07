mod body;
mod clause;
mod expr;

pub(crate) use aligned::*;
pub(crate) use boolean::*;
pub(crate) use cond::*;
pub(crate) use expr::*;
pub(crate) use function::*;
pub(crate) use paren::*;
pub(crate) use primary::*;
pub(crate) use subquery::*;

pub(crate) use body::*;
pub(crate) use clause::*;

use itertools::repeat_n;
use thiserror::Error;
use tree_sitter::{Node, Point, Range};

#[derive(Error, Debug)]
pub enum UroboroSQLFmtError {
    #[error("Illegal operation error: {0}")]
    IllegalOperation(String),
    #[error("Unexpected syntax error: {0}")]
    UnexpectedSyntax(String),
    #[error("Unimplemented Error: {0}")]
    Unimplemented(String),
    #[error("File not found error: {0}")]
    FileNotFound(String),
    #[error("Illegal setting file error: {0}")]
    IllegalSettingFile(String),
    #[error("Rendering Error: {0}")]
    Rendering(String),
    #[error("Runtime Error: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Position {
    pub(crate) row: usize,
    pub(crate) col: usize,
}

impl Position {
    pub(crate) fn new(point: Point) -> Position {
        Position {
            row: point.row,
            col: point.column,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Location {
    pub(crate) start_position: Position,
    pub(crate) end_position: Position,
}

impl Location {
    pub(crate) fn new(range: Range) -> Location {
        Location {
            start_position: Position::new(range.start_point),
            end_position: Position::new(range.end_point),
        }
    }
    // 隣り合っているか？
    pub(crate) fn is_next_to(&self, loc: &Location) -> bool {
        self.is_same_line(loc)
            && (self.end_position.col == loc.start_position.col
                || self.start_position.col == loc.end_position.col)
    }
    // 同じ行か？
    pub(crate) fn is_same_line(&self, loc: &Location) -> bool {
        self.end_position.row == loc.start_position.row
            || self.start_position.row == loc.end_position.row
    }

    // Locationのappend
    pub(crate) fn append(&mut self, loc: Location) {
        if self.end_position.row < loc.end_position.row
            || self.end_position.row == loc.end_position.row
                && self.end_position.col < loc.end_position.col
        {
            self.end_position = loc.end_position;
        }
    }

    /// Location が単一行を意味していれば true を返す
    pub(crate) fn is_single_line(&self) -> bool {
        self.start_position.row == self.end_position.row
    }
}

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

#[derive(Debug, Clone)]
pub(crate) struct Comment {
    text: String,
    loc: Location,
}

impl Comment {
    // tree_sitter::NodeオブジェクトからCommentオブジェクトを生成する
    pub(crate) fn new(node: Node, src: &str) -> Comment {
        Comment {
            text: node.utf8_text(src.as_bytes()).unwrap().to_string(),
            loc: Location::new(node.range()),
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// コメントが複数行コメントであればtrueを返す
    pub(crate) fn is_multi_line_comment(&self) -> bool {
        self.text.starts_with("/*")
    }

    /// コメントが/*_SQL_ID_*/であるかどうかを返す
    pub(crate) fn is_sql_id_comment(&self) -> bool {
        if self.text.starts_with("/*") {
            // 複数行コメント

            // コメントの中身を取り出す
            let content = self
                .text
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim();

            content == "_SQL_ID_" || content == "_SQL_IDENTIFIER_"
        } else {
            // 行コメント
            false
        }
    }

    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // インデントの挿入
        result.extend(repeat_n('\t', depth));

        if self.is_multi_line_comment() && self.loc.is_single_line() {
            // 元のコメントが、単一行のブロックコメントである場合、そのまま描画する
            result.push_str(&self.text);
        } else if self.is_multi_line_comment() {
            // multi lines

            let lines: Vec<_> = self
                .text
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim()
                .split('\n')
                .collect();

            result.push_str("/*\n");

            for line in &lines {
                let line = line.trim();
                result.extend(repeat_n('\t', depth + 1));
                result.push_str(line);
                result.push('\n');
            }

            result.extend(repeat_n('\t', depth));
            result.push_str("*/");
        } else {
            // single line
            result.push_str(&self.text);
        }

        Ok(result)
    }
}
