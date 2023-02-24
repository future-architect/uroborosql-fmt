use itertools::{repeat_n, Itertools};
use thiserror::Error;
use tree_sitter::{Node, Point, Range};

use crate::config::CONFIG;
use crate::util::*;

#[derive(Error, Debug)]
pub enum UroboroSQLFmtError {
    #[error("Illegal operation error: {0}")]
    IllegalOperationError(String),
    #[error("Unexpected syntax error: {0}")]
    UnexpectedSyntaxError(String),
    #[error("Unimplemented Error: {0}")]
    UnimplementedError(String),
    #[error("File not found error: {0}")]
    FileNotFoundError(String),
    #[error("Illegal setting file error: {0}")]
    IllegalSettingFileError(String),
    #[error("Rendering Error: {0}")]
    RenderingError(String),
}

/// 設定からタブ長を取得する
fn tab_size() -> usize {
    CONFIG.read().unwrap().tab_size
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
        self.end_position = loc.end_position;
    }
}

/// AlignedExprの演算子、コメントを縦ぞろえする際に使用する情報を含む構造体
#[derive(Debug)]
pub(crate) struct AlignInfo {
    /// 演算子自身の最長の長さ
    max_op_tab_num: Option<usize>,
    /// 演算子までの最長の長さ
    max_tab_num_to_op: Option<usize>,
    /// 行末コメントまでの最長の長さ
    max_tab_num_to_comment: Option<usize>,
}

impl From<Vec<&AlignedExpr>> for AlignInfo {
    /// AlignedExprのVecからAlignInfoを生成する
    fn from(aligned_exprs: Vec<&AlignedExpr>) -> Self {
        let has_op = aligned_exprs.iter().any(|aligned| aligned.has_rhs());

        let has_comment = aligned_exprs.iter().any(|aligned| {
            aligned.trailing_comment.is_some() || aligned.lhs_trailing_comment.is_some()
        });

        // 演算子自体の長さ
        let max_op_tab_num = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.op_tab_num().unwrap_or(0))
                .max()
        } else {
            None
        };

        let max_tab_num_to_op = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.lhs_tab_num())
                .max()
        } else {
            None
        };

        let max_tab_num_to_comment = if has_comment {
            aligned_exprs
                .iter()
                .flat_map(|aligned| aligned.tab_num_to_comment(max_tab_num_to_op))
                .max()
        } else {
            None
        };

        AlignInfo {
            max_op_tab_num,
            max_tab_num_to_op,
            max_tab_num_to_comment,
        }
    }
}

impl AlignInfo {
    fn new(
        max_op_tab_num: Option<usize>,
        max_tab_num_to_op: Option<usize>,
        max_tab_num_to_comment: Option<usize>,
    ) -> AlignInfo {
        AlignInfo {
            max_op_tab_num,
            max_tab_num_to_op,
            max_tab_num_to_comment,
        }
    }
}

// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub(crate) struct SeparatedLines {
    depth: usize,                               // インデントの深さ
    separator: String,                          // セパレータ(e.g., ',', AND)
    contents: Vec<(AlignedExpr, Vec<Comment>)>, // 各行の情報
    loc: Option<Location>,
    has_op: bool,       // 演算子があるかどうか
    is_from_body: bool, // render時にopを省略
}

impl SeparatedLines {
    pub(crate) fn new(depth: usize, sep: impl Into<String>, is_omit_op: bool) -> SeparatedLines {
        let separator = sep.into();
        SeparatedLines {
            depth,
            separator,
            contents: vec![] as Vec<(AlignedExpr, Vec<Comment>)>,
            loc: None,
            has_op: false,
            is_from_body: is_omit_op,
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

        if comment.is_multi_line_comment() || !self.loc().unwrap().is_same_line(&comment.loc()) {
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

    fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// AS句で揃えたものを返す
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // 演算子自体の長さ
        let align_info = self.contents.iter().map(|(a, _)| a).collect_vec().into();
        let mut is_first_line = true;

        for (aligned, comments) in &self.contents {
            result.extend(repeat_n('\t', self.depth));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(&self.separator);
            }
            result.push('\t');

            // alignedに演算子までの最長の長さを与えてフォーマット済みの文字列をもらう
            let formatted = aligned.render_align(self.depth, &align_info, self.is_from_body)?;
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

// *_statementに対応した構造体
#[derive(Debug, Clone)]
pub(crate) struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Location>,
    /// Statementの上に現れるコメント
    comments: Vec<Comment>,
    depth: usize,
    /// 末尾にセミコロンがついているか
    has_semi: bool,
}

impl Statement {
    pub(crate) fn new(depth: usize) -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
            comments: vec![] as Vec<Comment>,
            depth,
            has_semi: false,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
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

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        // clause1
        // ...
        // clausen
        let mut result = String::new();

        for comment in &self.comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        // 1つでもエラーの場合は全体もエラー
        for clause in &self.clauses {
            result.push_str(&clause.render()?);
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

        if self.text.starts_with("/*") {
            // multi lines

            let lines: Vec<_> = self
                .text
                .trim_start_matches("/*")
                .trim_end_matches("*/")
                .trim()
                .split('\n')
                .collect();

            result.extend(repeat_n('\t', depth));
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

            result.extend(repeat_n('\t', depth));
            result.push_str(&self.text);
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    Insert(InsertBody),
}

impl Body {
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
            Body::Insert(insert) => Some(insert.loc()),
        }
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(),
            Body::BooleanExpr(bool_expr) => bool_expr.render(),
            Body::Insert(insert) => insert.render(),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            Body::SepLines(sep_lines) => {
                sep_lines.add_comment_to_child(comment)?;
            }
            Body::BooleanExpr(bool_expr) => {
                bool_expr.add_comment_to_child(comment)?;
            }
            Body::Insert(insert) => {
                insert.add_comment_to_child(comment)?;
            }
        }

        Ok(())
    }

    // bodyの要素が空であるかどうかを返す
    fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::BooleanExpr(bool_expr) => bool_expr.is_empty(),
            Body::Insert(_) => false, // InsertBodyには必ずtable_nameが含まれる
        }
    }

    // 一つのExprからなるBodyを生成し返す
    pub(crate) fn with_expr(expr: Expr, depth: usize) -> Body {
        if expr.is_body() {
            // Bodyである場合はそのまま返せばよい
            if let Expr::Boolean(boolean) = expr {
                Body::BooleanExpr(*boolean)
            } else {
                // error
                unimplemented!()
            }
        } else {
            // Bodyでない場合、SeparatedLinesにして返す
            let mut sep_lines = SeparatedLines::new(depth, "", false);
            sep_lines.add_expr(expr.to_aligned());
            Body::SepLines(sep_lines)
        }
    }
}

/// 列リストを表す
/// VALUES句、SET句で使用する
#[derive(Debug, Clone)]
pub(crate) struct ColumnList {
    cols: Vec<Expr>,
    loc: Location,
}

impl ColumnList {
    pub(crate) fn new(cols: Vec<Expr>, loc: Location) -> ColumnList {
        ColumnList { cols, loc }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn last_line_len(&self) -> usize {
        // かっこ、カンマを考慮していないため、正確な値ではない
        self.cols
            .iter()
            .fold(0, |prev, e| prev + e.last_line_tab_num())
            * tab_size()
    }

    /// カラムリストをrenderする
    /// VALUES句以外(SET句)で呼び出された場合、1行で出力する
    /// depth: インデントの深さ。SET句では0が与えられる
    /// is_one_row: VALUES句で指定される行が一つであればtrue、そうでなければfalseであるような値
    fn render(&self, depth: usize, is_one_row: bool) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        if is_one_row {
            // ValuesItemが一つだけである場合、各列を複数行に出力する

            result.push_str(" (\n");

            // 最初の行のインデント
            result.extend(repeat_n('\t', depth));

            // 各要素間の改行、カンマ、インデント
            let mut separator = "\n,".to_string();
            separator.extend(repeat_n('\t', depth));

            result.push_str(
                &self
                    .cols
                    .iter()
                    .filter_map(|e| e.render().ok())
                    .join(&separator),
            );

            result.push('\n');
            result.extend(repeat_n('\t', depth - 1));
            result.push(')');
        } else {
            // ValuesItemが複数ある場合、各行は1行に出力する

            result.extend(repeat_n('\t', depth));
            result.push('(');
            result.push_str(&self.cols.iter().filter_map(|e| e.render().ok()).join(", "));
            result.push(')');
        }

        // 閉じかっこの後の改行は呼び出し元が担当
        Ok(result)
    }
}

/// INSERT文の本体
/// テーブル名、対象のカラム名、VALUES句を含む
#[derive(Debug, Clone)]
pub(crate) struct InsertBody {
    depth: usize,
    loc: Location,
    /// テーブル名
    table_name: AlignedExpr,
    /// カラム名
    column_name: Option<SeparatedLines>,
    /// VALUES句のキーワード(VALUESまたはDEFAULT VALUES)
    values_kw: Option<String>,
    /// VALUES句の本体
    values_body: Vec<ColumnList>,
}

impl InsertBody {
    pub(crate) fn new(depth: usize, loc: Location, table_name: AlignedExpr) -> InsertBody {
        InsertBody {
            depth,
            loc,
            table_name,
            column_name: None,
            values_kw: None,
            values_body: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// カラム名をセットする
    pub(crate) fn set_column_name(&mut self, cols: SeparatedLines) {
        self.column_name = Some(cols);
    }

    /// VALUES句をセットする
    pub(crate) fn set_values_clause(&mut self, kw: &str, body: Vec<ColumnList>) {
        self.values_kw = Some(kw.to_string());
        self.values_body = body;
    }

    /// 子供にコメントを追加する
    ///
    /// 対応済み
    /// - テーブル名の行末コメント
    ///
    /// 未対応
    /// - VALUES句の直後に現れるコメント
    /// - VALUES句の本体に現れるコメント
    /// - カラム名の直後に現れるコメント
    /// - テーブル名の直後に現れるコメント
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        // 下から順番に見ていく

        // table_nameの直後に現れる
        if comment.is_multi_line_comment() || !self.table_name.loc().is_same_line(&comment.loc()) {
            // 行末コメントではない場合は未対応
            unimplemented!()
        } else {
            // 行末コメントである場合、table_nameに追加する
            self.table_name.set_trailing_comment(comment)?;
        }

        Ok(())
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // テーブル名
        result.extend(repeat_n('\t', self.depth + 1));
        result.push_str(&self.table_name.render()?);
        result.push('\n');

        // カラム名
        if let Some(sep_lines) = &self.column_name {
            result.extend(repeat_n('\t', self.depth));
            result.push_str("(\n");
            result.push_str(&sep_lines.render()?);
            result.push(')');
        }

        // VALUES句
        if let Some(kw) = &self.values_kw {
            result.push(' ');
            result.push_str(&kw);

            // 要素が一つか二つ以上かでフォーマット方針が異なる
            let is_one_row = self.values_body.len() == 1;

            if !is_one_row {
                result.push('\n');
            }

            result.push_str(
                &self
                    .values_body
                    .iter()
                    .filter_map(|cols| cols.render(self.depth + 1, is_one_row).ok())
                    .join("\n,"),
            );
            result.push('\n');
        } else if self.column_name.is_some() {
            // VALUES句があるときは、改行を入れずに`VALUES`キーワードを出力している
            // そのため、VALUES句がない場合はここで改行
            result.push('\n');
        }

        Ok(result)
    }
}

// 句に対応した構造体
#[derive(Debug, Clone)]
pub(crate) struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<Body>,
    loc: Location,
    depth: usize,
    sql_id: Option<Comment>, // DML(, DDL)に付与できるsql_id
    comments: Vec<Comment>,  // キーワードの下に現れるコメント
}

impl Clause {
    pub(crate) fn new(kw_node: Node, src: &str, depth: usize) -> Clause {
        // コーディング規約によると、キーワードは大文字で記述する
        let keyword = format_keyword(kw_node.utf8_text(src.as_bytes()).unwrap());
        let loc = Location::new(kw_node.range());
        Clause {
            keyword,
            body: None,
            loc,
            depth,
            sql_id: None,
            comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// キーワードを延長する
    pub(crate) fn extend_kw(&mut self, node: Node, src: &str) {
        let loc = Location::new(node.range());
        self.loc.append(loc);
        self.keyword.push(' ');
        self.keyword
            .push_str(&format_keyword(node.utf8_text(src.as_bytes()).unwrap()));
    }

    // bodyをセットする
    pub(crate) fn set_body(&mut self, body: Body) {
        if !body.is_empty() {
            self.loc.append(body.loc().unwrap());
            self.body = Some(body);
        }
    }

    /// Clauseにコメントを追加する
    /// Bodyがあればその下にコメントを追加し、ない場合はキーワードの下にコメントを追加する
    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match &mut self.body {
            Some(body) if !body.is_empty() => {
                // bodyに式があれば、その下につく
                body.add_comment_to_child(comment)?;
            }
            _ => {
                // そうでない場合、自分のキーワードの下につく
                self.comments.push(comment);
            }
        }

        Ok(())
    }

    pub(crate) fn set_sql_id(&mut self, comment: Comment) {
        self.sql_id = Some(comment);
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        // kw
        // body...
        let mut result = String::new();

        result.extend(repeat_n('\t', self.depth));
        result.push_str(&self.keyword);

        if let Some(sql_id) = &self.sql_id {
            result.push(' ');
            result.push_str(&sql_id.text);
        }

        // comments
        for comment in &self.comments {
            result.push('\n');
            result.push_str(&comment.render(self.depth)?);
        }

        if let Some(sl) = &self.body {
            let formatted_body = sl.render()?;
            result.push('\n');
            result.push_str(&formatted_body);
        } else {
            result.push('\n');
        };

        Ok(result)
    }
}

// 式に対応した列挙体
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    /// AS句、二項比較演算、BETWEEN述語など、縦ぞろえを行う式
    Aligned(Box<AlignedExpr>),
    /// 識別子、文字列、数値など
    Primary(Box<PrimaryExpr>),
    /// bool式
    Boolean(Box<BooleanExpr>),
    /// SELECTサブクエリ
    SelectSub(Box<SelectSubExpr>),
    /// かっこでくくられた式
    ParenExpr(Box<ParenExpr>),
    /// アスタリスク*
    Asterisk(Box<AsteriskExpr>),
    /// CASE式
    Cond(Box<CondExpr>),
    /// 単項演算式(NOT, +, -, ...)
    Unary(Box<UnaryExpr>),
    /// カラムリスト(VALUES句、SET句)
    ColumnList(Box<ColumnList>),
    /// 関数呼び出し
    FunctionCall(Box<FunctionCall>),
}

impl Expr {
    pub(crate) fn loc(&self) -> Location {
        match self {
            Expr::Aligned(aligned) => aligned.loc(),
            Expr::Primary(primary) => primary.loc(),
            Expr::Boolean(sep_lines) => sep_lines.loc().unwrap(),
            Expr::SelectSub(select_sub) => select_sub.loc(),
            Expr::ParenExpr(paren_expr) => paren_expr.loc(),
            Expr::Asterisk(asterisk) => asterisk.loc(),
            Expr::Cond(cond) => cond.loc(),
            Expr::Unary(unary) => unary.loc(),
            Expr::ColumnList(cols) => cols.loc(),
            Expr::FunctionCall(func_call) => func_call.loc(),
        }
    }

    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        match self {
            Expr::Aligned(aligned) => {
                // 演算子を縦ぞろえしない場合は、ここでrender()が呼ばれる
                aligned.render()
            }
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
            Expr::ParenExpr(paren_expr) => paren_expr.render(),
            Expr::Asterisk(asterisk) => asterisk.render(),
            Expr::Cond(cond) => cond.render(),
            Expr::Unary(unary) => unary.render(),
            Expr::ColumnList(cols) => cols.render(0, false),
            Expr::FunctionCall(func_call) => func_call.render(), // _ => unimplemented!(),
        }
    }

    /// 最後の行の長さをタブ文字換算した結果を返す
    fn last_line_tab_num(&self) -> usize {
        to_tab_num(self.last_line_len())
    }

    /// 最後の行の文字列の長さを返す
    fn last_line_len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.last_line_len(),
            Expr::Aligned(aligned) => aligned.last_line_len(),
            Expr::SelectSub(_) => ")".len(), // 必ずかっこ
            Expr::ParenExpr(_) => ")".len(), // 必ずかっこ
            Expr::Asterisk(asterisk) => asterisk.last_line_len(),
            Expr::Cond(_) => "END".len(), // "END"
            Expr::Unary(unary) => unary.last_line_len(),
            Expr::ColumnList(cols) => cols.last_line_len(),
            Expr::FunctionCall(func_call) => func_call.last_line_len(),
            Expr::Boolean(_) => unimplemented!(),
        }
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        match self {
            // aligned, primaryは上位のExpr, Bodyでset_trailing_comment()を通じてコメントを追加する
            Expr::Aligned(_aligned) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for aligned",
                )));
            }
            Expr::Primary(_primary) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for primary",
                )));
            }

            // 下位の式にコメントを追加する
            Expr::Boolean(boolean) => {
                boolean.add_comment_to_child(comment)?;
            }
            Expr::SelectSub(select_sub) => select_sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => {
                paren_expr.add_comment_to_child(comment)?;
            }

            Expr::Cond(_cond) => {
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented for conditional_expr",
                )));
            }
            _ => {
                // todo
                return Err(UroboroSQLFmtError::UnimplementedError(format!(
                    "add_comment_to_child(): unimplemented expr",
                )));
            }
        }
        Ok(())
    }

    /// バインドパラメータをセットする
    /// コメントがバインドパラメータであるか(式と隣り合っているか)は呼び出し元で保証する
    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        match self {
            Expr::Primary(primary) => primary.set_head_comment(comment),
            Expr::Aligned(aligned) => aligned.set_head_comment(comment),
            Expr::Boolean(boolean) => boolean.set_head_comment(comment),
            // primary, aligned, boolean以外の式は現状、バインドパラメータがつくことはない
            _ => unimplemented!(),
        }
    }

    /// 複数行の式であればtrueを返す
    fn is_multi_line(&self) -> bool {
        match self {
            Expr::Boolean(_) | Expr::SelectSub(_) | Expr::ParenExpr(_) | Expr::Cond(_) => true,
            Expr::Primary(_) | Expr::Asterisk(_) => false,
            Expr::Aligned(aligned) => aligned.is_multi_line(),
            Expr::Unary(unary) => unary.is_multi_line(),
            Expr::FunctionCall(func_call) => func_call.is_multi_line(),
            Expr::ColumnList(_) => todo!(),
        }
    }

    // Bodyになる式(先頭のインデントと末尾の改行を行う式)であればtrue
    // そうでなければfalseを返す
    fn is_body(&self) -> bool {
        match self {
            Expr::Boolean(_) => true,
            Expr::Aligned(_)
            | Expr::Primary(_)
            | Expr::SelectSub(_)
            | Expr::ParenExpr(_)
            | Expr::Asterisk(_)
            | Expr::Cond(_)
            | Expr::Unary(_)
            | Expr::ColumnList(_)
            | Expr::FunctionCall(_) => false,
            // _ => unimplemented!(),
        }
    }

    /// 自身をAlignedExprでラッピングする
    pub(crate) fn to_aligned(&self) -> AlignedExpr {
        // TODO: cloneする必要があるか検討
        if let Expr::Aligned(aligned) = self {
            *aligned.clone()
        } else {
            let aligned = AlignedExpr::new(self.clone(), false);
            aligned
        }
    }
}

// 次を入れるとエラーになる
#[derive(Debug, Clone)]
pub(crate) struct AlignedExpr {
    lhs: Expr,
    rhs: Option<Expr>,
    op: Option<String>,
    loc: Location,
    trailing_comment: Option<String>,     // 行末コメント[
    lhs_trailing_comment: Option<String>, // 左辺の行末コメント
    is_alias: bool,
}

impl AlignedExpr {
    pub(crate) fn new(lhs: Expr, is_alias: bool) -> AlignedExpr {
        let loc = lhs.loc();
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            trailing_comment: None,
            lhs_trailing_comment: None,
            is_alias,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// opのタブ文字換算の長さを返す
    fn op_tab_num(&self) -> Option<usize> {
        self.op.as_ref().map(|op| to_tab_num(op.len()))
    }

    /// 最後の行の文字列の長さを返す
    fn last_line_len(&self) -> usize {
        match (&self.op, &self.rhs) {
            // 右辺があり、複数行ではない場合、(左辺'\t'演算子'\t'右辺) の長さを返す
            (Some(_), Some(rhs)) if !rhs.is_multi_line() => {
                (self.lhs.last_line_tab_num() + self.op_tab_num().unwrap()) * tab_size()
                    + rhs.last_line_len()
            }
            // 右辺があり、複数行である場合、右辺の長さを返す
            (Some(_), Some(rhs)) => rhs.last_line_len(),
            _ => self.lhs.last_line_len(),
        }
    }

    /// 右辺(行全体)のtrailing_commentをセットする
    /// 複数行コメントを与えた場合エラーを返す
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperationError(format!(
                "set_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            let Comment { text, loc } = comment;
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// 左辺のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_lhs_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            Err(UroboroSQLFmtError::IllegalOperationError(format!(
                "set_lhs_trailing_comment:{:?} is not trailing comment!",
                comment
            )))
        } else {
            // 行コメント
            let Comment { text, loc } = comment;
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.lhs_trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
            Ok(())
        }
    }

    /// 左辺にバインドパラメータをセットする
    /// 隣り合っているかどうかは呼び出しもとでチェック済み
    pub fn set_head_comment(&mut self, comment: Comment) {
        self.lhs.set_head_comment(comment);
    }

    // 演算子と右辺の式を追加する
    pub(crate) fn add_rhs(&mut self, op: impl Into<String>, rhs: Expr) {
        self.loc.append(rhs.loc());
        self.op = Some(op.into());
        self.rhs = Some(rhs);
    }

    // 右辺があるかどうかをboolで返す
    pub(crate) fn has_rhs(&self) -> bool {
        self.rhs.is_some()
    }

    /// 複数行であるかどうかを返す
    fn is_multi_line(&self) -> bool {
        self.lhs.is_multi_line() || self.rhs.as_ref().map(Expr::is_multi_line).unwrap_or(false)
    }

    // 演算子までの長さを返す
    // 左辺の長さを返せばよい
    pub(crate) fn lhs_tab_num(&self) -> usize {
        if self.lhs_trailing_comment.is_some() {
            // trailing commentが左辺にある場合、改行するため0
            0
        } else {
            self.lhs.last_line_tab_num()
        }
    }

    // 演算子から末尾コメントまでの長さを返す
    pub(crate) fn tab_num_to_comment(&self, max_tab_num_to_op: Option<usize>) -> Option<usize> {
        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        match (max_tab_num_to_op, &self.rhs) {
            // コメント以外にそろえる対象があり、この式が右辺を持つ場合は右辺の長さ
            (Some(_), Some(rhs)) => Some(rhs.last_line_tab_num()),
            // コメント以外に揃える対象があり、右辺を左辺で補完する場合、左辺の長さ
            (Some(_), None)
                if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk =>
            {
                if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = *strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right, primary.loc());
                    Some(new_prim.last_line_tab_num())
                } else {
                    Some(self.lhs.last_line_tab_num())
                }
            }
            // コメント以外に揃える対象があり、右辺を左辺を保管しない場合、0
            (Some(_), None) => Some(0),
            // そろえる対象がコメントだけであるとき、左辺の長さ
            _ => Some(self.lhs.last_line_tab_num()),
        }
    }

    /// 演算子・コメントの縦ぞろえをせずにrenderする
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let tab_num_to_op = if self.has_rhs() {
            Some(self.lhs_tab_num())
        } else {
            None
        };
        self.render_align(
            0,
            &AlignInfo::new(
                self.op_tab_num(),
                tab_num_to_op,
                self.tab_num_to_comment(tab_num_to_op),
            ),
            false,
        )
    }

    /// 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub(crate) fn render_align(
        &self,
        depth: usize,
        align_info: &AlignInfo,
        is_from_body: bool,
    ) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        let max_op_tab_num = align_info.max_op_tab_num;
        let max_tab_num_to_op = align_info.max_tab_num_to_op;
        let max_tab_num_to_comment = align_info.max_tab_num_to_comment;

        //左辺をrender
        let formatted = self.lhs.render()?;
        result.push_str(&formatted);

        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        // 演算子と右辺をrender
        match (&self.op, max_op_tab_num, max_tab_num_to_op) {
            (Some(op), Some(max_op_tab_num), Some(max_tab_num)) => {
                if let Some(comment_str) = &self.lhs_trailing_comment {
                    result.push('\t');
                    result.push_str(comment_str);
                    result.push('\n');

                    // インデントを挿入
                    result.extend(repeat_n('\t', depth));
                }

                let tab_num = max_tab_num - self.lhs_tab_num();
                result.extend(repeat_n('\t', tab_num));

                result.push('\t');

                // from句以外はopを挿入
                if !is_from_body {
                    result.push_str(op);
                    let tab_num = max_op_tab_num - self.op_tab_num().unwrap(); // self.op != Noneならop_tab_num != None
                    result.extend(repeat_n('\t', tab_num + 1));
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = rhs.render()?;
                    result.push_str(&formatted);
                }
            }
            // AS補完する場合
            (None, _, Some(max_tab_num))
                if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk =>
            {
                let tab_num = max_tab_num - self.lhs.last_line_tab_num();
                result.extend(repeat_n('\t', tab_num));

                if !is_from_body {
                    result.push('\t');
                    result.push_str(&format_keyword("AS"));
                }
                // エイリアス補完はすべての演算子が"AS"であるかないため、すべての演算子の長さ(op_tab_num())は等しい
                result.push('\t');

                let formatted = if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = *strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right, primary.loc());
                    new_prim.render().unwrap()
                } else {
                    self.lhs.render().unwrap()
                };

                result.push_str(&formatted);
            }
            (_, _, _) => (),
        }

        // 末尾コメントをrender
        match (&self.trailing_comment, max_op_tab_num, max_tab_num_to_op) {
            // 末尾コメントが存在し、ほかのそろえる対象が存在する場合
            (Some(comment), Some(max_op_tab_num), Some(max_tab_num)) => {
                let tab_num = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // trailing_commentがある場合、max_tab_num_to_commentは必ずSome(_)
                    max_tab_num_to_comment.unwrap() - rhs.last_line_tab_num()
                        + if rhs.is_multi_line() {
                            // 右辺が複数行である場合、最後の行に左辺と演算子はないため、その分タブで埋める
                            max_tab_num + max_op_tab_num
                        } else {
                            0
                        }
                } else if CONFIG.read().unwrap().complement_as && self.is_alias && !is_asterisk {
                    let lhs_tab_num = if let Expr::Primary(primary) = &self.lhs {
                        let str = primary.elements().first().unwrap();
                        let strs: Vec<&str> = str.split('.').collect();
                        let right = *strs.last().unwrap();
                        let new_prim = PrimaryExpr::new(right, primary.loc());
                        new_prim.last_line_tab_num()
                    } else {
                        self.lhs.last_line_tab_num()
                    };
                    // AS補完する場合には、右辺に左辺と同じ式を挿入する
                    max_tab_num_to_comment.unwrap() - lhs_tab_num
                } else {
                    // 右辺がない場合は
                    // コメントまでの最長 + 演算子の長さ + 左辺の最大長からの差分
                    max_tab_num_to_comment.unwrap()
                        + (if is_from_body { 0 } else { max_op_tab_num })
                        + max_tab_num
                        - self.lhs.last_line_tab_num()
                };

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            // 末尾コメントが存在し、ほかにはそろえる対象が存在しない場合
            (Some(comment), _, None) => {
                // max_tab_num_to_opがNoneであればそろえる対象はない
                let tab_num = max_tab_num_to_comment.unwrap() - self.lhs.last_line_tab_num();

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            _ => (),
        }

        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PrimaryExpr {
    elements: Vec<String>,
    loc: Location,
    head_comment: Option<String>,
}

impl PrimaryExpr {
    pub(crate) fn new(element: impl Into<String>, loc: Location) -> PrimaryExpr {
        PrimaryExpr {
            elements: vec![element.into()],
            loc,
            head_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_tab_num(&self) -> usize {
        to_tab_num(self.last_line_len())
    }

    pub(crate) fn last_line_len(&self) -> usize {
        // elementsをフォーマットするとき、各要素間に '\t' が挿入される
        //
        // e.g., TAB_SIZE = 4のとき
        // TAB1.NUM: 8文字 = TAB_SIZE * 2 -> tabを足すと長さTAB_SIZE * 2 + TAB_SIZE
        // TAB1.N  : 5文字 = TAB_SIZE * 1 + 1 -> tabを足すと長さTAB_SIZE + TAB_SIZE
        // -- 例外 --
        // N       : 1文字 < TAB_SIZE -> tabを入れると長さTAB_SIZE

        self.elements
            .iter()
            .map(String::len)
            .enumerate()
            .fold(0, |sum, (i, len)| {
                // 最初の要素には、バインドパラメータがつく可能性がある
                let len = match (i, &self.head_comment) {
                    (0, Some(head_comment)) => head_comment.len() + len,
                    _ => len,
                };

                // フォーマット時に、各elemの間にタブ文字が挿入される
                to_tab_num(sum) * tab_size() + len
            })
    }

    pub(crate) fn elements(&self) -> &Vec<String> {
        &self.elements
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment {
            text: mut comment,
            mut loc,
        } = comment;

        if CONFIG.read().unwrap().trim_bind_param {
            // 1. /*を削除
            // 2. *\を削除
            // 3. 前後の空白文字を削除
            // 4. /* */付与
            comment = format!(
                "/*{}*/",
                comment
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim()
            );
        }

        self.head_comment = Some(comment.clone());
        loc.append(self.loc.clone());
        self.loc = loc;
    }

    /// elementsにelementを追加する
    pub(crate) fn add_element(&mut self, element: &str) {
        self.elements.push(element.to_owned());
    }

    // PrimaryExprの結合
    pub(crate) fn append(&mut self, primary: PrimaryExpr) {
        self.elements.append(&mut primary.elements().clone())
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        // 文字列リテラル以外の要素を大文字に変換して、出力する文字列を生成する
        let elements_str = self
            .elements
            .iter()
            .map(|elem| to_uppercase_identifier(elem))
            .join("\t");

        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, elements_str)),
            None => Ok(elements_str),
        }
    }
}

// TODO: 大文字/小文字を設定ファイルで定義できるようにする
/// 引数の文字列が識別子であれば大文字にして返す
/// 文字列リテラル、または引用符付き識別子である場合はそのままの文字列を返す
fn to_uppercase_identifier(elem: &str) -> String {
    if (elem.starts_with("\"") && elem.ends_with("\""))
        || (elem.starts_with("'") && elem.ends_with("'"))
        || (elem.starts_with("$") && elem.ends_with("$"))
    {
        elem.to_owned()
    } else {
        elem.to_uppercase()
    }
}
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

    fn is_empty(&self) -> bool {
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

// SELECTサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub(crate) struct SelectSubExpr {
    depth: usize,
    stmt: Statement,
    loc: Location,
}

impl SelectSubExpr {
    pub(crate) fn new(stmt: Statement, loc: Location, depth: usize) -> SelectSubExpr {
        SelectSubExpr { depth, stmt, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(&mut self, _comment: Comment) {
        unimplemented!()
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.stmt.render()?;

        result.push_str(&formatted);

        result.extend(repeat_n('\t', self.depth));
        result.push(')');

        Ok(result)
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ParenExpr {
    depth: usize,
    expr: Expr,
    loc: Location,
    start_comments: Vec<Comment>,
    end_comments: Vec<Comment>,
}

impl ParenExpr {
    pub(crate) fn new(expr: Expr, loc: Location, depth: usize) -> ParenExpr {
        ParenExpr {
            depth,
            expr,
            loc,
            start_comments: vec![],
            end_comments: vec![],
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if self.expr.loc().is_same_line(&comment.loc()) {
            self.expr.add_comment_to_child(comment)?;
        } else {
            self.add_end_comment(comment);
        }

        Ok(())
    }

    pub(crate) fn set_loc(&mut self, loc: Location) {
        self.loc = loc;
    }

    // 開きかっこから最初の式の間に現れるコメントを追加する
    pub(crate) fn add_start_comment(&mut self, comment: Comment) {
        self.start_comments.push(comment);
    }

    // 最後の式から閉じかっこの間に現れるコメントを追加する
    pub(crate) fn add_end_comment(&mut self, comment: Comment) {
        self.end_comments.push(comment);
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str("(\n");

        for comment in &self.start_comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        let formatted = self.expr.render()?;

        // bodyでない式は、最初の行のインデントを自分で行わない。
        // そのため、かっこのインデントの深さ + 1個分インデントを挿入する。
        if !self.expr.is_body() {
            result.extend(repeat_n('\t', self.depth + 1));
        }

        result.push_str(&formatted);

        // インデント同様に、最後の改行も行う
        if !self.expr.is_body() {
            result.push('\n');
        }

        for comment in &self.end_comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        result.extend(repeat_n('\t', self.depth));
        result.push(')');
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AsteriskExpr {
    content: String,
    loc: Location,
}

impl AsteriskExpr {
    pub(crate) fn new(content: impl Into<String>, loc: Location) -> AsteriskExpr {
        let content = content.into();
        AsteriskExpr { content, loc }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn last_line_len(&self) -> usize {
        self.content.len()
    }

    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        Ok(self.content.clone())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CondExpr {
    depth: usize,
    when_then_clause: Vec<(Clause, Clause)>,
    else_clause: Option<Clause>,
    loc: Location,
    comments: Vec<Comment>, // CASEキーワードの後に現れるコメント
}

impl CondExpr {
    pub(crate) fn new(loc: Location, depth: usize) -> CondExpr {
        CondExpr {
            depth,
            when_then_clause: vec![],
            else_clause: None,
            loc,
            comments: vec![],
        }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_when_then_clause(&mut self, when_clause: Clause, then_clause: Clause) {
        self.when_then_clause.push((when_clause, then_clause));
    }

    pub(crate) fn set_else_clause(&mut self, else_clause: Clause) {
        self.else_clause = Some(else_clause);
    }

    /// 最後の式にコメントを追加する
    pub(crate) fn set_trailing_comment(
        &mut self,
        comment: Comment,
    ) -> Result<(), UroboroSQLFmtError> {
        if let Some(else_clause) = self.else_clause.as_mut() {
            else_clause.add_comment_to_child(comment)?;
        } else if let Some(when_then_expr) = self.when_then_clause.last_mut() {
            when_then_expr.1.add_comment_to_child(comment)?;
        } else {
            // when_then/else が存在しない場合
            // つまり、CASEキーワードの直後にコメントが来た場合
            self.comments.push(comment);
        }

        Ok(())
    }

    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        // CASEキーワードの行のインデントは呼び出し側が行う
        result.push_str(&format_keyword("CASE"));
        result.push('\n');

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

/// 単項演算式
/// e.g.,) NOT A, -B, ...
#[derive(Debug, Clone)]
pub(crate) struct UnaryExpr {
    operator: String,
    operand: Expr,
    loc: Location,
}

impl UnaryExpr {
    pub(crate) fn new(operator: impl Into<String>, operand: Expr, loc: Location) -> UnaryExpr {
        let operator = operator.into();
        UnaryExpr {
            operator,
            operand,
            loc,
        }
    }

    /// ソースコード上の位置を返す
    fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 演算子'\t'式 の最後の行の長さを返す
    fn last_line_len(&self) -> usize {
        if (&self.operand).is_multi_line() {
            self.operand.last_line_len()
        } else {
            to_tab_num(self.operator.len()) * tab_size() + self.operand.last_line_len()
        }
    }

    /// 複数行であるかどうかを返す
    fn is_multi_line(&self) -> bool {
        self.operand.is_multi_line()
    }

    /// フォーマットした文字列を返す
    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();

        result.push_str(&self.operator);
        result.push('\t');
        result.push_str(&self.operand.render()?);

        Ok(result)
    }
}

/// 関数呼び出し
#[derive(Debug, Clone)]
pub(crate) struct FunctionCall {
    name: String,
    args: Vec<Expr>,
    loc: Location,
    depth: usize,
}

impl FunctionCall {
    pub(crate) fn new(
        name: impl Into<String>,
        args: &[Expr],
        loc: Location,
        depth: usize,
    ) -> FunctionCall {
        let name = name.into();
        FunctionCall {
            name,
            args: args.to_vec(),
            loc,
            depth,
        }
    }

    /// 関数名'('引数')' の長さを返す
    /// 引数が複数行になる場合、')'の長さになる
    fn last_line_len(&self) -> usize {
        if self.is_multi_line() {
            ")".len()
        } else {
            let name_len = self.name.len();
            let args_len = self.args.len();
            let args_len: usize = self
                .args
                .iter()
                .map(|e| e.last_line_len())
                .fold(0, |sum, l| sum + l)
                + ", ".len() * (args_len - 1);

            let last_line_len = name_len + "(".len() + args_len + ")".len();

            last_line_len
        }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn is_multi_line(&self) -> bool {
        self.args.iter().any(|expr| expr.is_multi_line())
    }

    fn render(&self) -> Result<String, UroboroSQLFmtError> {
        let mut result = String::new();
        let func_name = to_uppercase_identifier(&self.name);

        result.push_str(&func_name);
        result.push('(');

        // arguments
        let args = self
            .args
            .iter()
            .map(|arg| arg.render())
            .collect::<Result<Vec<_>, _>>()?;

        if self.is_multi_line() {
            result.push('\n');

            let mut is_first = true;
            for arg in &args {
                // 関数呼び出しの深さ + 1 段インデントを挿入する
                result.extend(repeat_n('\t', self.depth + 1));
                if is_first {
                    is_first = false;
                } else {
                    result.push(',');
                }
                result.push('\t');
                result.push_str(arg);
                result.push('\n');
            }
            result.extend(repeat_n('\t', self.depth + 1));
        } else {
            result.push_str(&args.join(", "));
        }

        result.push(')');

        Ok(result)
    }
}

/// 引数をタブ数換算した値を返す
fn to_tab_num(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        len / tab_size() + 1
    }
}
