mod body;
mod clause;
mod expr;
mod statement;

pub(crate) use body::*;
pub(crate) use clause::*;
pub(crate) use expr::*;
pub(crate) use statement::*;

// expr
pub(crate) use aligned::*;
pub(crate) use asterisk::*;
pub(crate) use column_list::*;
pub(crate) use cond::*;
pub(crate) use conflict_target::*;
pub(crate) use expr_seq::*;
pub(crate) use function::*;
pub(crate) use paren::*;
pub(crate) use primary::*;
pub(crate) use subquery::*;

// body
pub(crate) use insert::*;
pub(crate) use separeted_lines::*;
pub(crate) use single_line::*;
pub(crate) use with::*;

use itertools::{repeat_n, Itertools};
use tree_sitter::{Node, Point, Range};

use crate::{config::CONFIG, error::UroboroSQLFmtError, re::RE};

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
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

    /// コメントがブロックコメントであればtrueを返す
    pub(crate) fn is_block_comment(&self) -> bool {
        self.text.starts_with("/*")
    }

    pub(crate) fn is_two_way_sql_comment(&self) -> bool {
        RE.branching_keyword_re.find(self.text.as_str()).is_some()
    }

    fn render(&self, depth: usize) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // インデントの挿入
        result.extend(repeat_n('\t', depth));

        if self.is_block_comment() && !self.loc.is_single_line() {
            // ブロックコメント かつ 単一行ではない (= 複数行ブロックコメント)

            // コメントの開始キーワード
            let start_keyword = if self.text.starts_with("/*+") {
                // ヒント句
                "/*+"
            } else {
                "/*"
            };
            // コメントの終了キーワード
            let end_keyword = "*/";

            // 開始キーワードと終了キーワードを除去して改行でsplit
            //
            // 以下のような例の場合、testの前の空白文字は保持される
            //
            // ```
            // /*
            //                  test
            // */
            // ```
            let lines = self
                .text
                .trim_start_matches(start_keyword) // 開始キーワードの除去
                .trim_end_matches(end_keyword) // 終了キーワードの除去
                .trim_end() // 終了キーワードの前のタブ/改行を除去
                .trim_start_matches(' ') // 開始キーワードの後の空白を除去
                .trim_start_matches('\n') // 開始キーワードの後の改行を除去
                .split('\n')
                .map(|line| line.trim_end()) // 各行の末尾の空白文字を除去
                .collect_vec();

            // 行の先頭のスペースの数をcount
            // タブも考慮する
            let count_start_space = |target: &str| {
                let mut res = 0;
                for c in target.chars() {
                    if c == '\t' {
                        res += CONFIG.read().unwrap().tab_size
                            - (res % CONFIG.read().unwrap().tab_size);
                    } else if c == ' ' {
                        res += 1;
                    } else {
                        break;
                    }
                }
                res
            };

            // 全ての行のうち先頭のスペースの数が最小のもの
            // ただし、空白行は無視
            let min_start_space = lines
                .iter()
                .filter(|x| !x.is_empty())
                .map(|x| count_start_space(x))
                .min()
                .unwrap_or(0);

            // 開始キーワードを描画して改行
            result.push_str(&format!("{start_keyword}\n"));

            // 各行を描画
            for line in &lines {
                // 必要な深さ
                let need_depth = depth + 1;

                // 設定ファイルに記述されたタブサイズ (デフォルトサイズ: 4)
                let tab_size = CONFIG.read().unwrap().tab_size;

                if line.is_empty() {
                    // 空白行の場合そのまま描画
                    result.push_str(line);
                } else if need_depth * tab_size >= min_start_space {
                    // タブが少なく、補完が必要な場合

                    // 必要なスペースの数
                    let need_space = need_depth * tab_size - min_start_space;

                    // 補完するスペースの数
                    let complement_tab = need_space / tab_size;

                    // 補完するスペースの数
                    let complement_space = if need_space % tab_size == 0 {
                        // 割り切れる場合はタブのみで補完
                        0
                    } else {
                        // 割り切れない場合はタブで補完した後にスペースで補完
                        need_space % tab_size
                    };

                    result.extend(repeat_n('\t', complement_tab));
                    result.extend(repeat_n(' ', complement_space));
                    result.push_str(line);
                } else {
                    // TABが多い場合

                    // 余分なスペース
                    let extra_space = min_start_space - (need_depth * tab_size);

                    let mut trimmed_line = line.to_string();

                    // 削除したスペース/タブの和
                    let mut removal_space = 0;

                    let mut pre_space_count = count_start_space(&trimmed_line);

                    // 余分な先頭のスペース/タブを除去する
                    loop {
                        trimmed_line = trimmed_line[1..].to_string();

                        let current_space_count = count_start_space(&trimmed_line);

                        // 削除したスペース/タブの和に今回削除したスペース/タブの数を追加
                        removal_space += pre_space_count - current_space_count;

                        pre_space_count = current_space_count;

                        // 削除したスペースの数が余分なスペースの数を超えたら終了
                        if removal_space >= extra_space {
                            break;
                        }
                    }

                    if removal_space > extra_space {
                        // 削除しすぎたスペースを追加
                        result.extend(repeat_n(' ', removal_space - extra_space));
                    }

                    result.push_str(&trimmed_line);
                }
                result.push('\n');
            }

            result.extend(repeat_n('\t', depth));
            result.push_str(end_keyword);
        } else {
            // 1行コメント
            result.push_str(&self.text);
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SqlID {
    sql_id: String,
}

impl SqlID {
    pub(crate) fn new(sql_id: String) -> SqlID {
        SqlID { sql_id }
    }

    /// /*_SQL_ID_*/であるかどうかを返す
    pub(crate) fn is_sql_id(text: &str) -> bool {
        if text.starts_with("/*") {
            // 複数行コメント

            // コメントの中身を取り出す
            let content = text.trim_start_matches("/*").trim_end_matches("*/").trim();

            content == "_SQL_ID_" || content == "_SQL_IDENTIFIER_"
        } else {
            // 行コメント
            false
        }
    }
}
