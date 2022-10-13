use itertools::{repeat_n, Itertools};
use tree_sitter::{Point, Range};

const TAB_SIZE: usize = 4; // タブ幅
const OPERATOR_TAB_NUM: usize = 1; // 演算子のタブ長
const PAR_TAB_NUM: usize = 1; // 閉じ括弧のタブ長

const COMPLEMENT_AS: bool = true; // AS句がない場合に自動的に補完する

const TRIM_BIND_PARAM: bool = false; // バインド変数の中身をトリムする

#[derive(Debug)]
pub(crate) enum Error {
    ParseError,
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

// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub(crate) struct SeparatedLines {
    depth: usize,               // インデントの深さ
    separator: String,          // セパレータ(e.g., ',', AND)
    contents: Vec<AlignedExpr>, // 各行の情報
    loc: Option<Location>,
    has_op: bool,       // 演算子があるかどうか
    is_from_body: bool, // render時にopを省略
}

impl SeparatedLines {
    pub(crate) fn new(depth: usize, sep: &str, is_omit_op: bool) -> SeparatedLines {
        SeparatedLines {
            depth,
            separator: sep.to_string(),
            contents: vec![] as Vec<AlignedExpr>,
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

        self.contents.push(aligned);
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        self.contents
            .last_mut()
            .unwrap()
            .set_trailing_comment(comment);
    }

    /// AS句で揃えたものを返す
    pub(crate) fn render(&self) -> Result<String, Error> {
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
pub(crate) struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Location>,
}

impl Default for Statement {
    fn default() -> Self {
        Self::new()
    }
}

impl Statement {
    pub(crate) fn new() -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    // 文に句を追加する
    pub(crate) fn add_clause(&mut self, clause: Clause) {
        match &mut self.loc {
            Some(loc) => loc.append(clause.loc()),
            None => self.loc = Some(clause.loc()),
        }
        self.clauses.push(clause);
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        self.clauses
            .last_mut()
            .unwrap()
            .add_comment_to_child(comment);
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
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
pub(crate) struct Comment {
    comment: String,
    loc: Location,
}

impl Comment {
    pub(crate) fn new(comment: String, loc: Location) -> Comment {
        Comment { comment, loc }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
    ParenExpr(ParenExpr),
}

impl Body {
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
            Body::ParenExpr(paren_expr) => Some(paren_expr.loc()),
        }
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(),
            Body::BooleanExpr(bool_expr) => bool_expr.render(),
            Body::ParenExpr(paren_expr) => paren_expr.render(),
        }
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment),
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment),
            Body::ParenExpr(paren_expr) => paren_expr.add_comment_to_child(comment),
        }
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
}

impl Clause {
    pub(crate) fn new(keyword: String, loc: Location, depth: usize) -> Clause {
        Clause {
            keyword,
            body: None,
            loc,
            depth,
            sql_id: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    // bodyをセットする
    pub(crate) fn set_body(&mut self, body: Body) {
        self.loc.append(body.loc().unwrap());
        self.body = Some(body);
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        if let Some(body) = &mut self.body {
            body.add_comment_to_child(comment);
        }
    }

    pub(crate) fn set_sql_id(&mut self, comment: Comment) {
        self.sql_id = Some(comment);
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        // kw
        // body...
        let mut result = String::new();

        result.extend(repeat_n('\t', self.depth));
        result.push_str(&self.keyword);

        if let Some(sql_id) = &self.sql_id {
            result.push(' ');
            result.push_str(&sql_id.comment);
        }

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
pub(crate) enum Expr {
    Aligned(Box<AlignedExpr>), // AS句、二項比較演算
    Primary(Box<PrimaryExpr>), // 識別子、文字列、数値など
    Boolean(Box<BooleanExpr>), // boolean式
    SelectSub(Box<SelectSubExpr>),
    ParenExpr(Box<ParenExpr>),
    Asterisk(Box<AsteriskExpr>),
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
        }
    }

    fn render(&self) -> Result<String, Error> {
        match self {
            Expr::Aligned(_aligned) => todo!(),
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
            Expr::ParenExpr(paren_expr) => paren_expr.render(),
            Expr::Asterisk(asterisk) => asterisk.render(),
        }
    }

    // 最後の行の長さをタブ文字換算した結果を返す
    fn len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.len(),
            Expr::SelectSub(_) => PAR_TAB_NUM, // 必ずかっこ
            Expr::ParenExpr(_) => PAR_TAB_NUM, // 必ずかっこ
            Expr::Asterisk(asterisk) => asterisk.len(),
            _ => todo!(),
        }
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Expr::Aligned(aligned) => aligned.set_trailing_comment(comment),
            Expr::Primary(_primary) => (),
            Expr::Boolean(boolean) => boolean.add_comment_to_child(comment),
            Expr::SelectSub(select_sub) => select_sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => paren_expr.add_comment_to_child(comment),
            _ => todo!(),
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
pub(crate) struct AlignedExpr {
    lhs: Expr,
    rhs: Option<Expr>,
    op: Option<String>,
    loc: Location,
    trailing_comment: Option<String>, // 行末コメント
    is_alias: bool,
}

impl AlignedExpr {
    pub(crate) fn new(lhs: Expr, loc: Location, is_alias: bool) -> AlignedExpr {
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            trailing_comment: None,
            is_alias,
        }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn set_trailing_comment(&mut self, comment: Comment) {
        let Comment { comment, loc } = comment;
        if comment.starts_with("/*") {
            self.trailing_comment = Some(comment);
        } else {
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", comment.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
        }

        self.loc.append(loc);
    }

    // 演算子と右辺の式を追加する
    pub(crate) fn add_rhs(&mut self, op: String, rhs: Expr) {
        self.loc.append(rhs.loc());
        self.op = Some(op);
        self.rhs = Some(rhs);
    }

    // 右辺があるかどうかをboolで返す
    pub(crate) fn has_rhs(&self) -> bool {
        self.rhs.is_some()
    }

    // 演算子までの長さを返す
    pub(crate) fn len_lhs(&self) -> usize {
        // 左辺の長さを返せばよい
        self.lhs.len()
    }

    // 演算子から末尾コメントまでの長さを返す
    pub(crate) fn len_to_comment(&self, max_len_to_op: Option<usize>) -> Option<usize> {
        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        match (max_len_to_op, &self.rhs) {
            // コメント以外にそろえる対象があり、この式が右辺を持つ場合は右辺の長さ
            (Some(_), Some(rhs)) => Some(rhs.len()),
            // コメント以外に揃える対象があり、右辺を左辺で補完する場合、左辺の長さ
            (Some(_), None) if COMPLEMENT_AS && self.is_alias && !is_asterisk => {
                if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right.to_string(), primary.loc());
                    Some(new_prim.len())
                } else {
                    Some(self.lhs.len())
                }
            }
            // コメント以外に揃える対象があり、右辺を左辺を保管しない場合、0
            (Some(_), None) => Some(0),
            // そろえる対象がコメントだけであるとき、左辺の長さ
            _ => Some(self.lhs.len()),
        }
    }

    // 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub(crate) fn render(
        &self,
        max_len_to_op: Option<usize>,
        max_len_to_comment: Option<usize>,
        is_from_body: bool,
    ) -> Result<String, Error> {
        let mut result = String::new();

        //左辺をrender
        let formatted = self.lhs.render()?;
        result.push_str(&formatted);

        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        // 演算子と右辺をrender
        match (&self.op, max_len_to_op) {
            (Some(op), Some(max_len)) => {
                let tab_num = max_len - self.lhs.len();
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
            (None, Some(max_len)) if COMPLEMENT_AS && self.is_alias && !is_asterisk => {
                let tab_num = max_len - self.lhs.len();
                result.extend(repeat_n('\t', tab_num));

                if !is_from_body {
                    result.push('\t');
                    result.push_str("AS");
                }

                result.push('\t');

                let formatted = if let Expr::Primary(primary) = &self.lhs {
                    let str = primary.elements().first().unwrap();
                    let strs: Vec<&str> = str.split('.').collect();
                    let right = strs.last().unwrap();
                    let new_prim = PrimaryExpr::new(right.to_string(), primary.loc());
                    new_prim.render().unwrap()
                } else {
                    self.lhs.render().unwrap()
                };

                result.push_str(&formatted);
            }
            (_, _) => (),
        }

        // 末尾コメントをrender
        match (&self.trailing_comment, max_len_to_op) {
            // 末尾コメントが存在し、ほかのそろえる対象が存在する場合
            (Some(comment), Some(max_len)) => {
                let tab_num = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // trailing_commentがある場合、max_len_to_commentは必ずSome(_)
                    max_len_to_comment.unwrap() - rhs.len()
                        + if rhs.is_multi_line() {
                            max_len + OPERATOR_TAB_NUM
                        } else {
                            0
                        }
                } else if COMPLEMENT_AS && self.is_alias && !is_asterisk {
                    let lhs_len = if let Expr::Primary(primary) = &self.lhs {
                        let str = primary.elements().first().unwrap();
                        let strs: Vec<&str> = str.split('.').collect();
                        let right = strs.last().unwrap();
                        let new_prim = PrimaryExpr::new(right.to_string(), primary.loc());
                        new_prim.len()
                    } else {
                        self.lhs.len()
                    };
                    // AS補完する場合には、右辺に左辺と同じ式を挿入する
                    max_len_to_comment.unwrap() - lhs_len
                } else {
                    // 右辺がない場合は
                    // コメントまでの最長 + TAB_SIZE(演算子の分) + 左辺の最大長からの差分
                    max_len_to_comment.unwrap()
                        + (if is_from_body { 0 } else { OPERATOR_TAB_NUM })
                        + max_len
                        - self.lhs.len()
                };

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
pub(crate) struct PrimaryExpr {
    elements: Vec<String>,
    loc: Location,
    len: usize,
    head_comment: Option<String>,
}

impl PrimaryExpr {
    pub(crate) fn new(element: String, loc: Location) -> PrimaryExpr {
        let len = element.len() / TAB_SIZE + 1;
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

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn elements(&self) -> &Vec<String> {
        &self.elements
    }

    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
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

        self.len += head_comment_and_first_element_len - first_element_len;
    }

    /// elementsにelementを追加する
    pub(crate) fn add_element(&mut self, element: &str) {
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
        self.len += element.len() / TAB_SIZE + 1;
        self.elements.push(element.to_ascii_uppercase());
    }

    // PrimaryExprの結合
    pub(crate) fn append(&mut self, primary: PrimaryExpr) {
        self.len += primary.len();
        self.elements.append(&mut primary.elements().clone())
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        let elements_str = self.elements.iter().map(|x| x.to_uppercase()).join("\t");

        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, elements_str)),
            None => Ok(elements_str),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ContentWithSep {
    separator: String,
    content: AlignedExpr,
}

#[derive(Debug, Clone)]
pub(crate) struct BooleanExpr {
    depth: usize,                  // インデントの深さ
    default_separator: String,     // デフォルトセパレータ(e.g., ',', AND)
    contents: Vec<ContentWithSep>, // {sep, contents}
    loc: Option<Location>,
    has_op: bool,
}

impl BooleanExpr {
    pub(crate) fn new(depth: usize, sep: &str) -> BooleanExpr {
        BooleanExpr {
            depth,
            default_separator: sep.to_string(),
            contents: vec![] as Vec<ContentWithSep>,
            loc: None,
            has_op: false,
        }
    }

    pub(crate) fn loc(&self) -> Option<Location> {
        self.loc.clone()
    }

    pub(crate) fn set_default_separator(&mut self, sep: String) {
        self.default_separator = sep;
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        self.contents
            .last_mut()
            .unwrap()
            .content
            .set_trailing_comment(comment);
    }

    pub(crate) fn add_expr_with_sep(&mut self, aligned: AlignedExpr, sep: String) {
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

    pub(crate) fn add_expr(&mut self, expr: AlignedExpr) {
        self.add_expr_with_sep(expr, self.default_separator.clone());
    }

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
    pub(crate) fn render(&self) -> Result<String, Error> {
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

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        self.stmt.add_comment_to_child(comment);
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
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
}

impl ParenExpr {
    pub(crate) fn new(expr: Expr, loc: Location, depth: usize) -> ParenExpr {
        ParenExpr { depth, expr, loc }
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        self.expr.add_comment_to_child(comment);
    }

    pub(crate) fn set_loc(&mut self, loc: Location) {
        self.loc = loc;
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        result.push_str("(\n");

        let formatted = self.expr.render()?;

        result.push_str(&formatted);

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
    pub(crate) fn new(content: String, loc: Location) -> AsteriskExpr {
        AsteriskExpr { content, loc }
    }

    fn loc(&self) -> Location {
        self.loc.clone()
    }

    fn len(&self) -> usize {
        self.content.len() / TAB_SIZE + 1
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        Ok(self.content.clone())
    }
}
