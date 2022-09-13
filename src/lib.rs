use itertools::{repeat_n, Itertools};
use tree_sitter::{Node, Point, Range};

const TAB_SIZE: usize = 4; // タブ幅

const COMPLEMENT_AS: bool = true; // AS句がない場合に自動的に補完する

const TRIM_BIND_PARAM: bool = false; // バインド変数の中身をトリムする

pub const DEBUG_MODE: bool = false; // デバッグモード

pub const COMMENT: &str = "comment";

/// 引数のSQLをフォーマットして返す
pub fn format_sql(src: &str) -> String {
    // tree-sitter-sqlの言語を取得
    let language = tree_sitter_sql::language();
    // パーサオブジェクトを生成
    let mut parser = tree_sitter::Parser::new();
    // tree-sitter-sqlの言語をパーサにセットする
    parser.set_language(language).unwrap();
    // srcをパースし、結果のTreeを取得
    let tree = parser.parse(&src, None).unwrap();
    // Treeのルートノードを取得
    let root_node = tree.root_node();

    // フォーマッタオブジェクトを生成
    let mut formatter = Formatter::default();

    // formatを行い、バッファに結果を格納
    let res = formatter.format_sql(root_node, src.as_ref());

    if DEBUG_MODE {
        eprintln!("{:#?}", res);
    }

    match res.render() {
        Ok(res) => res,
        Err(e) => panic!("{:?}", e),
    }
}

#[derive(Debug)]
pub enum Error {
    ParseError,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}

impl Position {
    pub fn new(point: Point) -> Position {
        Position {
            row: point.row,
            col: point.column,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Location {
    pub start_position: Position,
    pub end_position: Position,
}

impl Location {
    pub fn new(range: Range) -> Location {
        Location {
            start_position: Position::new(range.start_point),
            end_position: Position::new(range.end_point),
        }
    }
    // 隣り合っているか？
    // 同じ行か？
    pub fn is_same_line(&self, loc: &Location) -> bool {
        self.end_position.row == loc.start_position.row
            || self.start_position.row == loc.end_position.row
    }

    // Locationのappend
    pub fn append(&mut self, loc: Location) {
        self.end_position = loc.end_position;
    }
}

// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub struct SeparatedLines {
    depth: usize,               // インデントの深さ
    separator: String,          // セパレータ(e.g., ',', AND)
    contents: Vec<AlignedExpr>, // 各行の情報
    loc: Option<Location>,
    has_op: bool,       // 演算子があるかどうか
    is_from_body: bool, // render時にopを省略
}

impl SeparatedLines {
    pub fn new(depth: usize, sep: &str, is_omit_op: bool) -> SeparatedLines {
        SeparatedLines {
            depth,
            separator: sep.to_string(),
            contents: vec![] as Vec<AlignedExpr>,
            loc: None,
            has_op: false,
            is_from_body: is_omit_op,
        }
    }

    pub fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // 式を追加する
    pub fn add_expr(&mut self, aligned: AlignedExpr) {
        // 演算子があるかどうかをチェック
        if aligned.has_rhs() {
            self.has_op = true;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        self.contents.push(aligned);
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        self.contents.last_mut().unwrap().set_tail_comment(comment);
    }

    /// AS句で揃えたものを返す
    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        let max_len_to_op = if self.has_op {
            self.contents.iter().map(AlignedExpr::len_lhs).max()
        } else {
            // そろえる演算子がない場合はNone
            None
        };

        let max_len_to_comment = self
            .contents
            .iter()
            .flat_map(|aligned| aligned.len_to_comment(max_len_to_op))
            .max();

        let mut is_first_line = true;

        for aligned in (&self.contents).iter() {
            result.extend(repeat_n('\t', self.depth));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(&self.separator);
            }
            result.push('\t');

            // alignedに演算子までの最長の長さを与えてフォーマット済みの文字列をもらう
            let formatted = aligned.render(max_len_to_op, max_len_to_comment, self.is_from_body)?;
            result.push_str(&formatted);
            result.push('\n')
        }

        Ok(result)
    }
}

// *_statementに対応した構造体
#[derive(Debug, Clone)]
pub struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Location>,
}

impl Default for Statement {
    fn default() -> Self {
        Self::new()
    }
}

impl Statement {
    pub fn new() -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
        }
    }

    pub fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // 文に句を追加する
    pub fn add_clause(&mut self, clause: Clause) {
        match &mut self.loc {
            Some(loc) => loc.append(clause.loc()),
            None => self.loc = Some(clause.loc()),
        }
        self.clauses.push(clause);
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        let last_idx = self.clauses.len() - 1;
        self.clauses[last_idx].add_comment_to_child(comment);
    }

    pub fn render(&self) -> Result<String, Error> {
        // clause1
        // ...
        // clausen

        // 1つでもエラーの場合は全体もエラー
        self.clauses
            .iter()
            .map(Clause::render)
            .collect::<Result<String, Error>>()
    }
}

#[derive(Debug, Clone)]
pub struct Comment {
    comment: String,
    loc: Location,
}

impl Comment {
    pub fn new(comment: String, loc: Location) -> Comment {
        Comment { comment, loc }
    }
}

#[derive(Debug, Clone)]
pub enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    ParenExpr(ParenExpr),
}

impl Body {
    pub fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
            Body::ParenExpr(paren_expr) => Some(paren_expr.loc()),
        }
    }

    pub fn render(&self) -> Result<String, Error> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(),
            Body::BooleanExpr(bool_expr) => bool_expr.render(),
            Body::ParenExpr(paren_expr) => paren_expr.render(),
        }
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment),
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment),
            Body::ParenExpr(paren_expr) => paren_expr.add_comment_to_child(comment),
        }
    }
}

// 句に対応した構造体
#[derive(Debug, Clone)]
pub struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<Body>,
    loc: Location,
    depth: usize,
}

impl Clause {
    pub fn new(keyword: String, loc: Location, depth: usize) -> Clause {
        Clause {
            keyword,
            body: None,
            loc,
            depth,
        }
    }

    pub fn loc(&self) -> Location {
        self.loc.clone()
    }

    // bodyをセットする
    pub fn set_body(&mut self, body: Body) {
        self.loc.append(body.loc().unwrap());
        self.body = Some(body);
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        if let Some(body) = &mut self.body {
            body.add_comment_to_child(comment);
        }
    }

    pub fn render(&self) -> Result<String, Error> {
        // kw
        // body...
        let mut result = String::new();

        result.extend(repeat_n('\t', self.depth));

        result.push_str(&self.keyword);

        if let Some(sl) = &self.body {
            let formatted_body = sl.render()?;
            result.push('\n');
            result.push_str(&formatted_body);
        };

        Ok(result)
    }
}

// 式に対応した列挙体
#[derive(Debug, Clone)]
pub enum Expr {
    Aligned(Box<AlignedExpr>), // AS句、二項比較演算
    Primary(Box<PrimaryExpr>), // 識別子、文字列、数値など
    Boolean(Box<BooleanExpr>), // boolean式
    SelectSub(Box<SelectSubExpr>),
    ParenExpr(Box<ParenExpr>),
}

impl Expr {
    fn loc(&self) -> Location {
        match self {
            Expr::Aligned(aligned) => aligned.loc(),
            Expr::Primary(primary) => primary.loc(),
            Expr::Boolean(sep_lines) => sep_lines.loc().unwrap(),
            Expr::SelectSub(select_sub) => select_sub.loc(),
            Expr::ParenExpr(paren_expr) => paren_expr.loc(),
        }
    }

    fn render(&self) -> Result<String, Error> {
        match self {
            Expr::Aligned(_aligned) => todo!(),
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
            Expr::ParenExpr(paren_expr) => paren_expr.render(),
        }
    }

    // 最後の行の長さをタブ文字換算した結果を返す
    fn len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.len(),
            Expr::SelectSub(_) => TAB_SIZE, // 必ずかっこなので、TAB_SIZE
            Expr::ParenExpr(_) => TAB_SIZE, // 必ずかっこなので、TAB_SIZE
            _ => todo!(),
        }
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Expr::Aligned(aligned) => aligned.set_tail_comment(comment),
            Expr::Primary(_primary) => (),
            Expr::Boolean(boolean) => boolean.add_comment_to_child(comment),
            Expr::SelectSub(select_sub) => select_sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => paren_expr.add_comment_to_child(comment),
        }
    }

    fn is_multi_line(&self) -> bool {
        match self {
            Expr::Boolean(_) | Expr::SelectSub(_) => true,
            Expr::Primary(_) => false,
            _ => todo!(),
        }
    }
}

// 次を入れるとエラーになる
#[derive(Debug, Clone)]
pub struct AlignedExpr {
    lhs: Expr,
    rhs: Option<Expr>,
    op: Option<String>,
    loc: Location,
    tail_comment: Option<String>, // 行末コメント
    is_alias: bool,
}

impl AlignedExpr {
    pub fn new(lhs: Expr, loc: Location, is_alias: bool) -> AlignedExpr {
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            tail_comment: None,
            is_alias,
        }
    }

    pub fn lhs(&self) -> Expr {
        self.lhs.clone()
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub fn set_tail_comment(&mut self, comment: Comment) {
        let Comment { comment, loc } = comment;
        if comment.starts_with("/*") {
            self.tail_comment = Some(comment);
        } else {
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let tail_comment = format!("-- {}", comment.trim_start_matches('-').trim_start());

            self.tail_comment = Some(tail_comment);
        }

        self.loc.append(loc);
    }

    // 演算子と右辺の式を追加する
    pub fn add_rhs(&mut self, op: String, rhs: Expr) {
        self.loc.append(rhs.loc());
        self.op = Some(op);
        self.rhs = Some(rhs);
    }

    // 右辺があるかどうかをboolで返す
    pub fn has_rhs(&self) -> bool {
        self.rhs.is_some()
    }

    // 演算子までの長さを返す
    pub fn len_lhs(&self) -> usize {
        // 左辺の長さを返せばよい
        self.lhs.len()
    }

    // 演算子から末尾コメントまでの長さを返す
    pub fn len_to_comment(&self, max_len_to_op: Option<usize>) -> Option<usize> {
        match (max_len_to_op, &self.rhs) {
            // コメント以外にそろえる対象があり、この式が右辺を持つ場合は右辺の長さ
            (Some(_), Some(rhs)) => Some(rhs.len()),
            // コメント以外に揃える対象があり、右辺を左辺で補完する場合、左辺の長さ
            (Some(_), None) if COMPLEMENT_AS && self.is_alias => Some(self.lhs.len()),
            // コメント以外に揃える対象があり、右辺を左辺を保管しない場合、0
            (Some(_), None) => Some(0),
            // そろえる対象がコメントだけであるとき、左辺の長さ
            _ => Some(self.lhs.len()),
        }
    }

    // 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub fn render(
        &self,
        max_len_to_op: Option<usize>,
        max_len_to_comment: Option<usize>,
        is_from_body: bool,
    ) -> Result<String, Error> {
        let mut result = String::new();

        //左辺をrender
        let formatted = self.lhs.render()?;
        result.push_str(&formatted);

        // 演算子と右辺をrender
        match (&self.op, max_len_to_op) {
            (Some(op), Some(max_len)) => {
                let tab_num = (max_len - self.lhs.len()) / TAB_SIZE;
                result.extend(repeat_n('\t', tab_num));

                result.push('\t');

                // from句以外はopを挿入
                if !is_from_body {
                    result.push_str(op);
                    result.push('\t');
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = rhs.render()?;
                    result.push_str(&formatted);
                }
            }
            // AS補完する場合
            (None, Some(max_len)) if COMPLEMENT_AS && self.is_alias => {
                let tab_num = (max_len - self.lhs.len()) / TAB_SIZE;
                result.extend(repeat_n('\t', tab_num));

                if !is_from_body {
                    result.push('\t');
                    result.push_str("AS");
                }

                result.push('\t');
                let formatted = self.lhs.render().unwrap();
                result.push_str(&formatted);
            }
            (_, _) => (),
        }

        // 末尾コメントをrender
        match (&self.tail_comment, max_len_to_op) {
            // 末尾コメントが存在し、ほかのそろえる対象が存在する場合
            (Some(comment), Some(max_len)) => {
                let tab_num = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // tail_commentがある場合、max_len_to_commentは必ずSome(_)
                    max_len_to_comment.unwrap() - rhs.len()
                        + if rhs.is_multi_line() {
                            max_len + TAB_SIZE
                        } else {
                            0
                        }
                } else if COMPLEMENT_AS && self.is_alias {
                    // AS補完する場合には、右辺に左辺と同じ式を挿入する
                    max_len_to_comment.unwrap() - self.lhs.len()
                } else {
                    // 右辺がない場合は
                    // コメントまでの最長 + TAB_SIZE(演算子の分) + 左辺の最大長からの差分
                    max_len_to_comment.unwrap()
                        + (if is_from_body { 0 } else { TAB_SIZE })
                        + max_len
                        - self.lhs.len()
                } / TAB_SIZE;

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            // 末尾コメントが存在し、ほかにはそろえる対象が存在しない場合
            (Some(comment), None) => {
                let tab_num = (max_len_to_comment.unwrap() - self.lhs.len()) / TAB_SIZE;

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
pub struct PrimaryExpr {
    elements: Vec<String>,
    loc: Location,
    len: usize,
    head_comment: Option<String>,
}

impl PrimaryExpr {
    pub fn new(element: String, loc: Location) -> PrimaryExpr {
        let len = TAB_SIZE * (element.len() / TAB_SIZE + 1);
        PrimaryExpr {
            elements: vec![element],
            loc,
            len,
            head_comment: None,
        }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.elements.len() == 0
    }

    pub fn elements(&self) -> &Vec<String> {
        &self.elements
    }

    pub fn set_head_comment(&mut self, comment: Comment) {
        let Comment {
            mut comment,
            mut loc,
        } = comment;

        if TRIM_BIND_PARAM {
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

        let first_element_len = self.elements()[0].len() / TAB_SIZE + 1;
        let head_comment_and_first_element_len =
            (self.elements()[0].len() + comment.len()) / TAB_SIZE + 1;

        self.len += TAB_SIZE * (head_comment_and_first_element_len - first_element_len);
    }

    /// elementsにelementを追加する
    pub fn add_element(&mut self, element: &str) {
        // TAB_SIZEを1単位として長さを記録する
        //
        // contentを文字列にするとき、必ずその前に一つ'\t'が入る
        // -> 各contentの長さは content + "\t"となる
        //
        // e.g., TAB_SIZE = 4のとき
        // TAB1.NUM: 8文字 = TAB_SIZE * 2 -> tabを足すと長さTAB_SIZE * 2 + TAB_SIZE
        // TAB1.N  : 5文字 = TAB_SIZE * 1 + 1 -> tabを足すと長さTAB_SIZE + TAB_SIZE
        // -- 例外 --
        // N       : 1文字 < TAB_SIZE -> tabを入れると長さTAB_SIZE
        //
        self.len += TAB_SIZE * (element.len() / TAB_SIZE + 1);
        self.elements.push(element.to_ascii_uppercase());
    }

    // PrimaryExprの結合
    pub fn append(&mut self, primary: PrimaryExpr) {
        self.len += primary.len();
        self.elements.append(&mut primary.elements().clone())
    }

    pub fn render(&self) -> Result<String, Error> {
        let elements_str = self.elements.iter().map(|x| x.to_uppercase()).join("\t");

        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, elements_str)),
            None => Ok(elements_str),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentWithSep {
    separator: String,
    content: AlignedExpr,
}

#[derive(Debug, Clone)]
pub struct BooleanExpr {
    depth: usize,                  // インデントの深さ
    default_separator: String,     // デフォルトセパレータ(e.g., ',', AND)
    contents: Vec<ContentWithSep>, // {sep, contents}
    loc: Option<Location>,
    has_op: bool,
}

impl BooleanExpr {
    pub fn new(depth: usize, sep: &str) -> BooleanExpr {
        BooleanExpr {
            depth,
            default_separator: sep.to_string(),
            contents: vec![] as Vec<ContentWithSep>,
            loc: None,
            has_op: false,
        }
    }

    pub fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    pub fn set_default_separator(&mut self, sep: String) {
        self.default_separator = sep;
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        let last_idx = self.contents.len() - 1;
        self.contents[last_idx].content.set_tail_comment(comment);
    }

    pub fn add_expr_with_sep(&mut self, aligned: AlignedExpr, sep: String) {
        if aligned.has_rhs() {
            self.has_op = true;
        }

        // locationの更新
        match &mut self.loc {
            Some(loc) => loc.append(aligned.loc()),
            None => self.loc = Some(aligned.loc()),
        };

        self.contents.push(ContentWithSep {
            separator: sep,
            content: aligned,
        });
    }

    pub fn add_expr(&mut self, expr: AlignedExpr) {
        self.add_expr_with_sep(expr, self.default_separator.clone());
    }

    pub fn merge(&mut self, other: BooleanExpr) {
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
        for ContentWithSep { separator, content } in (other.contents).into_iter() {
            if is_first_content {
                self.add_expr_with_sep(content, self.default_separator.clone());
                is_first_content = false;
            } else {
                self.add_expr_with_sep(content, separator);
            }
        }
    }

    /// 比較演算子で揃えたものを返す
    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        let max_len_to_op = if self.has_op {
            self.contents
                .iter()
                .map(|pair| pair.content.len_lhs())
                .max()
        } else {
            None
        };

        // コメントまでの最長の長さを計算する
        let max_len_to_comment = self
            .contents
            .iter()
            .flat_map(|pair| pair.content.len_to_comment(max_len_to_op))
            .max();

        let mut is_first_line = true;

        for ContentWithSep { separator, content } in (&self.contents).iter() {
            result.extend(repeat_n('\t', self.depth));

            if is_first_line {
                is_first_line = false;
            } else {
                result.push_str(separator);
            }
            result.push('\t');

            let formatted = content.render(max_len_to_op, max_len_to_comment, false)?;
            result.push_str(&formatted);
            result.push('\n')
        }

        Ok(result)
    }
}

// SELECTサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub struct SelectSubExpr {
    depth: usize,
    stmt: Statement,
    loc: Location,
}

impl SelectSubExpr {
    pub fn new(stmt: Statement, loc: Location, depth: usize) -> SelectSubExpr {
        SelectSubExpr { depth, stmt, loc }
    }

    pub fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        self.stmt.add_comment_to_child(comment);
    }

    pub fn render(&self) -> Result<String, Error> {
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
pub struct ParenExpr {
    depth: usize,
    expr: Expr,
    loc: Location,
}

impl ParenExpr {
    pub fn new(expr: Expr, loc: Location, depth: usize) -> ParenExpr {
        ParenExpr { depth, expr, loc }
    }

    pub fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        self.expr.add_comment_to_child(comment);
    }

    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.expr.render()?;

        result.push_str(&formatted);

        result.extend(repeat_n('\t', self.depth));

        result.push(')');
        Ok(result)
    }
}

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub depth: usize,
}

pub struct Formatter {
    state: FormatterState,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub fn new() -> Formatter {
        Formatter {
            state: FormatterState { depth: 0 },
        }
    }

    /// sqlソースファイルをフォーマットし、bufに入れる
    pub fn format_sql(&mut self, node: Node, src: &str) -> Statement {
        self.format_source(node, src)
    }

    // ネストを1つ深くする
    fn nest(&mut self) {
        self.state.depth += 1;
    }

    // ネストを1つ浅くする
    fn unnest(&mut self) {
        self.state.depth -= 1;
    }

    fn format_source(&mut self, node: Node, src: &str) -> Statement {
        // source_file -> _statement*

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let stmt_node = cursor.node();

            // 現状はselect_statementのみ
            // 文が増えたらマッチ式で分岐させる
            let mut stmt = self.format_select_stmt(stmt_node, src);

            if cursor.goto_next_sibling() && cursor.node().kind() == COMMENT {
                let comment_loc = Location::new(cursor.node().range());

                //同じ行の場合
                if comment_loc.is_same_line(&stmt.loc().unwrap()) {
                    stmt.add_comment_to_child(Comment::new(
                        cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                        comment_loc,
                    ));
                } else {
                    //違う行の場合
                }
            }
            stmt
        } else {
            todo!()
        }
    }

    // SELECT文
    fn format_select_stmt(&mut self, node: Node, src: &str) -> Statement {
        /*
            _select_statement ->
                select_clause
                from_clause?
                where_clause?
        */

        let mut statement = Statement::default();

        let mut cursor = node.walk(); // cursor -> select_statement
        if cursor.goto_first_child() {
            // cursor -> select_clause
            let select_clause_node = cursor.node();

            statement.add_clause(self.format_select_clause(select_clause_node, src));
        }

        loop {
            // 次の兄弟へ移動
            // select_statementの子供がいなくなったら終了
            if !cursor.goto_next_sibling() {
                break;
            }

            let clause_node = cursor.node();
            // println!("{}", clause_node.kind());

            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // cursor -> FROM
                        let from_node = cursor.node();
                        let mut clause = Clause::new(
                            "FROM".to_string(),
                            Location::new(from_node.range()),
                            self.state.depth,
                        );

                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", true);

                        // commaSep
                        if cursor.goto_next_sibling() {
                            // cursor -> _aliasable_expression
                            let expr_node = cursor.node();

                            separated_lines.add_expr(self.format_aliasable_expr(expr_node, src));

                            while cursor.goto_next_sibling() {
                                // cursor -> , または cursor -> _aliasable_expression
                                let child_node = cursor.node();

                                match child_node.kind() {
                                    "," => continue,
                                    COMMENT => {
                                        let comment_loc = Location::new(child_node.range());

                                        //同じ行の場合
                                        if comment_loc.is_same_line(&separated_lines.loc().unwrap())
                                        {
                                            separated_lines.add_comment_to_child(Comment::new(
                                                child_node
                                                    .utf8_text(src.as_bytes())
                                                    .unwrap()
                                                    .to_string(),
                                                comment_loc,
                                            ));
                                        } else {
                                            //違う行の場合
                                        }
                                    }
                                    _ => {
                                        let alias = self.format_aliasable_expr(child_node, src);
                                        separated_lines.add_expr(alias);
                                    }
                                };
                            }
                        }

                        cursor.goto_parent();

                        clause.set_body(Body::SepLines(separated_lines));

                        statement.add_clause(clause);
                    }
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    if cursor.goto_first_child() {
                        // cursor -> WHERE
                        let where_node = cursor.node();
                        let mut clause = Clause::new(
                            "WHERE".to_string(),
                            Location::new(where_node.range()),
                            self.state.depth,
                        );

                        cursor.goto_next_sibling();
                        // cursor -> _expression

                        let expr_node = cursor.node();
                        let expr = self.format_expr(expr_node, src);

                        let body = match expr {
                            Expr::Aligned(aligned) => {
                                let mut separated_lines =
                                    SeparatedLines::new(self.state.depth, "", false);
                                separated_lines.add_expr(*aligned);
                                Body::SepLines(separated_lines)
                            }
                            Expr::Primary(_) => {
                                todo!();
                            }
                            Expr::Boolean(boolean) => Body::BooleanExpr(*boolean),
                            Expr::SelectSub(_select_sub) => todo!(),
                            Expr::ParenExpr(paren_expr) => Body::ParenExpr(*paren_expr),
                        };

                        cursor.goto_parent();

                        clause.set_body(body);

                        statement.add_clause(clause);
                    }
                }
                COMMENT => {
                    let comment_loc = Location::new(clause_node.range());

                    //同じ行の場合
                    if comment_loc.is_same_line(&statement.loc().unwrap()) {
                        statement.add_comment_to_child(Comment::new(
                            clause_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                            comment_loc,
                        ));
                    } else {
                        //違う行の場合
                    }
                }
                _ => {
                    break;
                }
            }
        }

        statement
    }

    // SELECT句
    fn format_select_clause(&mut self, node: Node, src: &str) -> Clause {
        /*
            select_clause ->
                "SELECT"
                select_clause_body
        */
        let mut cursor = node.walk(); // cursor -> select_clause

        let mut clause = Clause::new(
            "SELECT".to_string(),
            Location::new(node.range()),
            self.state.depth,
        );

        if cursor.goto_first_child() {
            // cursor -> SELECT
            // SELECTを読み飛ばす(コメントを考える際に変更予定)

            // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            cursor.goto_next_sibling();
            // cursor -> select_caluse_body

            let body = self.format_select_clause_body(cursor.node(), src);
            clause.set_body(Body::SepLines(body));
        }

        clause
    }

    // SELECT句の本体をSeparatedLinesで返す
    fn format_select_clause_body(&mut self, node: Node, src: &str) -> SeparatedLines {
        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        let mut cursor = node.walk(); // cursor -> select_clause_body

        cursor.goto_first_child();
        // cursor -> _aliasable_expression

        let expr_node = cursor.node();

        let mut separated_lines = SeparatedLines::new(self.state.depth, ",", false);

        let aligned = self.format_aliasable_expr(expr_node, src);
        separated_lines.add_expr(aligned);

        // (',' _aliasable_expression)*
        // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
        while cursor.goto_next_sibling() {
            // cursor -> , または cursor -> _aliasable_expression
            let child_node = cursor.node();
            match child_node.kind() {
                "," => continue,
                COMMENT => {
                    separated_lines.add_comment_to_child(Comment::new(
                        child_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                        Location::new(child_node.range()),
                    ));
                }
                _ => {
                    let aligned = self.format_aliasable_expr(child_node, src);
                    separated_lines.add_expr(aligned);
                }
            }
        }

        separated_lines
    }

    // エイリアス可能な式
    fn format_aliasable_expr(&mut self, node: Node, src: &str) -> AlignedExpr {
        /*
            _aliasable_expression ->
                alias | _expression

            alias ->
                _expression
                "AS"?
                identifier
                << 未対応!! "(" identifier ("," identifier)* ")" >>
        */
        match node.kind() {
            "alias" => {
                let mut cursor = node.walk();
                // cursor -> alias

                cursor.goto_first_child();
                // cursor -> _expression

                // _expression
                let lhs_expr = self.format_expr(cursor.node(), src);
                let lhs_expr_loc = lhs_expr.loc();

                let mut aligned = AlignedExpr::new(lhs_expr, lhs_expr_loc, true);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() {
                    // cursor -> "AS"?

                    // ASが存在する場合は読み飛ばす
                    if cursor.node().kind() == "AS" {
                        cursor.goto_next_sibling();
                    }

                    //右辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    if cursor.node().kind() == "identifier" {
                        let rhs = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        let rhs_expr =
                            PrimaryExpr::new(rhs.to_string(), Location::new(cursor.node().range()));
                        aligned.add_rhs("AS".to_string(), Expr::Primary(Box::new(rhs_expr)));
                    }
                }
                aligned
            }
            _ => {
                // _expression
                let expr_node = node;
                let expr = self.format_expr(expr_node, src);
                let expr_loc = expr.loc();

                AlignedExpr::new(expr, expr_loc, true)
            }
        }
    }

    // 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        matches!(
            op_str,
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*"
        )
    }

    // 式
    fn format_expr(&mut self, node: Node, src: &str) -> Expr {
        let mut cursor = node.walk();

        match cursor.node().kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                // cursor -> dotted_name

                let range = node.range();

                cursor.goto_first_child();

                // cursor -> identifier

                let mut dotted_name = String::new();

                let id_node = cursor.node();
                dotted_name.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                // while self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src) {
                while cursor.goto_next_sibling() {
                    // cursor -> . または cursor -> identifier
                    match cursor.node().kind() {
                        "." => dotted_name.push('.'),
                        _ => dotted_name.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(dotted_name, Location::new(range));

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                // cursor -> binary_expression

                cursor.goto_first_child();
                // cursor -> _expression

                // 左辺
                let lhs_node = cursor.node();
                let lhs_expr = self.format_expr(lhs_node, src);

                // self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                cursor.goto_next_sibling();
                // cursor -> op (e.g., "+", "-", "=", ...)

                // 演算子
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                cursor.goto_next_sibling();
                // cursor -> _expression

                let mut head_comment: Option<Comment> = None;
                if cursor.node().kind() == COMMENT {
                    let comment_loc = Location::new(cursor.node().range());
                    head_comment = Some(Comment {
                        comment: cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                        loc: comment_loc,
                    });

                    cursor.goto_next_sibling();
                }

                // 右辺
                let rhs_node = cursor.node();
                let mut rhs_expr = self.format_expr(rhs_node, src);

                match head_comment {
                    Some(comment) => match &mut rhs_expr {
                        Expr::Aligned(_) => todo!(),
                        Expr::Primary(primary) => primary.set_head_comment(comment),
                        Expr::Boolean(_) => todo!(),
                        Expr::SelectSub(_) => todo!(),
                        Expr::ParenExpr(_) => todo!(),
                    },
                    None => (),
                }

                if Self::is_comp_op(op_str) {
                    // 比較演算子 -> AlignedExpr
                    let lhs_loc = lhs_expr.loc();
                    let mut aligned = AlignedExpr::new(lhs_expr, lhs_loc, false);
                    aligned.add_rhs(op_str.to_string(), rhs_expr);

                    Expr::Aligned(Box::new(aligned))
                } else {
                    // 比較演算子でない -> PrimaryExpr
                    // e.g.,) 1 + 1
                    match lhs_expr {
                        Expr::Primary(mut lhs) => {
                            lhs.add_element(op_str);
                            match rhs_expr {
                                Expr::Primary(rhs) => lhs.append(*rhs),
                                _ => {
                                    // 右辺が複数行の場合
                                    todo!()
                                }
                            }
                            Expr::Primary(lhs)
                        }
                        _ => {
                            // 左辺が複数行の場合
                            todo!()
                        }
                    }
                }
            }
            "boolean_expression" => self.format_bool_expr(node, src),
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let primary = PrimaryExpr::new(
                    node.utf8_text(src.as_bytes()).unwrap().to_string(),
                    Location::new(node.range()),
                );

                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(node, src);
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
            }
            "parenthesized_expression" => {
                let paren_expr = self.format_paren_expr(node, src);
                Expr::ParenExpr(Box::new(paren_expr))
            }

            _ => {
                eprintln!(
                    "format_expr(): unimplemented expression {}, {:#?}",
                    cursor.node().kind(),
                    cursor.node().range()
                );
                todo!()
            }
        }
    }

    fn format_bool_expr(&mut self, node: Node, src: &str) -> Expr {
        /*
        boolean_expression: $ =>
            choice(
            prec.left(PREC.unary, seq(kw("NOT"), $._expression)),
            prec.left(PREC.and, seq($._expression, kw("AND"), $._expression)),
            prec.left(PREC.or, seq($._expression, kw("OR"), $._expression)),
        ),
         */

        let mut boolean_expr = BooleanExpr::new(self.state.depth, "-");

        let mut cursor = node.walk();

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            todo!();
        } else {
            let left = self.format_expr(cursor.node(), src);
            match left {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
                Expr::ParenExpr(paren_expr) => {
                    let loc = paren_expr.loc();
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), loc, false);
                    boolean_expr.add_expr(aligned);
                }
            }

            cursor.goto_next_sibling();

            if cursor.node().kind() == COMMENT {
                let comment_loc = Location::new(cursor.node().range());
                boolean_expr.add_comment_to_child(Comment::new(
                    cursor.node().utf8_text(src.as_bytes()).unwrap().to_string(),
                    comment_loc,
                ));
                cursor.goto_next_sibling();
            }

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(sep.to_string());

            cursor.goto_next_sibling();
            let right = self.format_expr(cursor.node(), src);

            match right {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
                Expr::ParenExpr(paren_expr) => {
                    let loc = paren_expr.loc();
                    let aligned = AlignedExpr::new(Expr::ParenExpr(paren_expr), loc, false);
                    boolean_expr.add_expr(aligned);
                }
            }
        }
        Expr::Boolean(Box::new(boolean_expr))
    }

    fn format_select_subexpr(&mut self, node: Node, src: &str) -> SelectSubExpr {
        // select_subexpression -> "(" select_statement ")"

        let loc = Location::new(node.range());

        let mut cursor = node.walk(); // cursor -> select_subexpression

        cursor.goto_first_child();
        // cursor -> (
        // 将来的には、かっこの数を数えるかもしれない

        cursor.goto_next_sibling();
        // cursor -> select_statement

        self.nest();
        let select_stmt_node = cursor.node();
        let select_stmt = self.format_select_stmt(select_stmt_node, src);
        self.unnest();

        cursor.goto_next_sibling();
        // cursor -> )

        SelectSubExpr::new(select_stmt, loc, self.state.depth)
    }

    fn format_paren_expr(&mut self, node: Node, src: &str) -> ParenExpr {
        let mut cursor = node.walk();

        cursor.goto_first_child();
        //cursor -> "("

        cursor.goto_next_sibling();
        //cursor -> expr

        let mut is_nest = false;
        match cursor.node().kind() {
            "parenthesized_expression" => (),
            _ => is_nest = true,
        }

        if is_nest {
            self.nest();
        }

        let expr = self.format_expr(cursor.node(), src);

        cursor.goto_next_sibling();
        //cursor -> ")"

        match expr {
            Expr::ParenExpr(paren_expr) => *paren_expr,
            _ => {
                let loc = expr.loc();
                let paren_expr = ParenExpr::new(expr, loc, self.state.depth);
                self.unnest();
                paren_expr
            }
        }
    }
}

// cstを表示する関数(デバッグ用)
pub fn print_cst(src: &str) {
    // tree-sitter-sqlの言語を取得
    let language = tree_sitter_sql::language();
    // パーサオブジェクトを生成
    let mut parser = tree_sitter::Parser::new();
    // tree-sitter-sqlの言語をパーサにセットする
    parser.set_language(language).unwrap();

    // srcをパースし、結果のTreeを取得
    let tree = parser.parse(&src, None).unwrap();
    // Treeのルートノードを取得
    let root_node = tree.root_node();

    if DEBUG_MODE {
        dfs(root_node, 0);
        eprintln!();
    }
}

fn dfs(node: Node, depth: usize) {
    for _ in 0..depth {
        eprint!("\t");
    }
    eprint!(
        "{} [{}-{}]",
        node.kind(),
        node.start_position(),
        node.end_position()
    );

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            eprintln!();
            dfs(cursor.node(), depth + 1);
            //次の兄弟ノードへカーソルを移動
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
