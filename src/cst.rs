use itertools::{repeat_n, Itertools};
use tree_sitter::{Node, Point, Range};

const TAB_SIZE: usize = 4; // タブ幅
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
    max_len_op: Option<usize>,
    /// 演算子までの最長の長さ
    max_len_to_op: Option<usize>,
    /// 行末コメントまでの最長の長さ
    max_len_to_comment: Option<usize>,
}

impl From<Vec<&AlignedExpr>> for AlignInfo {
    /// AlignedExprのVecからAlignInfoを生成する
    fn from(aligned_exprs: Vec<&AlignedExpr>) -> Self {
        let has_op = aligned_exprs.iter().any(|aligned| aligned.has_rhs());

        let has_comment = aligned_exprs.iter().any(|aligned| {
            aligned.trailing_comment.is_some() || aligned.lhs_trailing_comment.is_some()
        });

        // 演算子自体の長さ
        let max_len_op = if has_op {
            aligned_exprs
                .iter()
                .map(|aligned| aligned.len_op().unwrap_or(0))
                .max()
        } else {
            None
        };

        let max_len_to_op = if has_op {
            aligned_exprs.iter().map(|aligned| aligned.len_lhs()).max()
        } else {
            None
        };

        let max_len_to_comment = if has_comment {
            aligned_exprs
                .iter()
                .flat_map(|aligned| aligned.len_to_comment(max_len_to_op))
                .max()
        } else {
            None
        };

        AlignInfo {
            max_len_op,
            max_len_to_op,
            max_len_to_comment,
        }
    }
}

impl AlignInfo {
    fn new(
        max_len_op: Option<usize>,
        max_len_to_op: Option<usize>,
        max_len_to_comment: Option<usize>,
    ) -> AlignInfo {
        AlignInfo {
            max_len_op,
            max_len_to_op,
            max_len_to_comment,
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
    pub(crate) fn new(depth: usize, sep: &str, is_omit_op: bool) -> SeparatedLines {
        SeparatedLines {
            depth,
            separator: sep.to_string(),
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

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
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
                .set_trailing_comment(comment);
        }
    }

    fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// AS句で揃えたものを返す
    pub(crate) fn render(&self) -> Result<String, Error> {
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
            let formatted = aligned.render(self.depth, &align_info, self.is_from_body)?;
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
    comments: Vec<Comment>, // Statementの上に現れるコメント
    depth: usize,
}

impl Statement {
    pub(crate) fn new(depth: usize) -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
            comments: vec![] as Vec<Comment>,
            depth,
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

    // Statementの上に現れるコメントを追加する
    pub(crate) fn add_comment(&mut self, comment: Comment) {
        self.comments.push(comment);
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
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

    fn render(&self, depth: usize) -> Result<String, Error> {
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
}

impl Body {
    pub(crate) fn loc(&self) -> Option<Location> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
        }
    }

    pub(crate) fn render(&self) -> Result<String, Error> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(),
            Body::BooleanExpr(bool_expr) => bool_expr.render(),
        }
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment),
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment),
        }
    }

    // bodyの要素が空であるかどうかを返す
    fn is_empty(&self) -> bool {
        match self {
            Body::SepLines(sep_lines) => sep_lines.is_empty(),
            Body::BooleanExpr(bool_expr) => bool_expr.is_empty(),
        }
    }

    // 一つのExprからなるBodyを生成し返す
    pub(crate) fn new_body_with_expr(expr: Expr, depth: usize) -> Body {
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
            match expr {
                Expr::Aligned(aligned) => sep_lines.add_expr(*aligned),
                _ => {
                    // Bodyでなく、AlignedExprでもない場合、AlignedExprでラッピングしてSeparatedLinesに
                    let aligned = AlignedExpr::new(expr, false);
                    sep_lines.add_expr(aligned);
                }
            }
            Body::SepLines(sep_lines)
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
    comments: Vec<Comment>,  // キーワードの下に現れるコメント
}

impl Clause {
    pub(crate) fn new(keyword: String, loc: Location, depth: usize) -> Clause {
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

    // bodyをセットする
    pub(crate) fn set_body(&mut self, body: Body) {
        self.loc.append(body.loc().unwrap());
        self.body = Some(body);
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        match &mut self.body {
            Some(body) if !body.is_empty() => body.add_comment_to_child(comment), // bodyに式があれば、その下につく
            _ => self.comments.push(comment), // そうでない場合、自分のキーワードの下につく
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
    Cond(Box<CondExpr>),
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
            // _ => unimplemented!(),
        }
    }

    fn render(&self) -> Result<String, Error> {
        match self {
            Expr::Aligned(aligned) => {
                // 演算子を縦ぞろえしない場合は、ここでrender()が呼ばれる

                let len_to_op = if aligned.has_rhs() {
                    Some(aligned.len_lhs())
                } else {
                    None
                };
                let len_op = aligned.len_op();
                aligned.render(
                    0,
                    &AlignInfo::new(len_op, len_to_op, aligned.len_to_comment(len_to_op)),
                    false,
                )
            }
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
            Expr::ParenExpr(paren_expr) => paren_expr.render(),
            Expr::Asterisk(asterisk) => asterisk.render(),
            Expr::Cond(cond) => cond.render(),
            // _ => unimplemented!(),
        }
    }

    /// 最後の行の長さをタブ文字換算した結果を返す
    fn len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.len(),
            Expr::Aligned(aligned) => aligned.len(),
            Expr::SelectSub(_) => PAR_TAB_NUM, // 必ずかっこ
            Expr::ParenExpr(_) => PAR_TAB_NUM, // 必ずかっこ
            Expr::Asterisk(asterisk) => asterisk.len(),
            Expr::Cond(_) => PAR_TAB_NUM, // "END"
            _ => unimplemented!(),
        }
    }

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            // aligned, primaryは上位のExpr, Bodyでset_trailing_comment()を通じてコメントを追加する
            Expr::Aligned(_aligned) => unimplemented!(),
            Expr::Primary(_primary) => unimplemented!(),

            // 下位の式にコメントを追加する
            Expr::Boolean(boolean) => boolean.add_comment_to_child(comment),
            Expr::SelectSub(select_sub) => select_sub.add_comment_to_child(comment),
            Expr::ParenExpr(paren_expr) => paren_expr.add_comment_to_child(comment),

            Expr::Cond(_cond) => unimplemented!(),
            _ => todo!(),
        }
    }

    /// 複数行の式であればtrueを返す
    fn is_multi_line(&self) -> bool {
        match self {
            Expr::Boolean(_) | Expr::SelectSub(_) | Expr::ParenExpr(_) | Expr::Cond(_) => true,
            Expr::Primary(_) => false,
            Expr::Aligned(aligned) => aligned.is_multi_line(),
            _ => todo!("is_multi_line: {:?}", self),
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
            | Expr::Cond(_) => false,
            // _ => unimplemented!(),
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
    fn len_op(&self) -> Option<usize> {
        self.op.as_ref().map(|op| op.len() / TAB_SIZE + 1)
    }

    /// 最後の行の長さを返す
    fn len(&self) -> usize {
        match (&self.op, &self.rhs) {
            (Some(_), Some(rhs)) => {
                // 右辺が存在する場合、右辺が複数行かどうかで決まる
                if !rhs.is_multi_line() {
                    self.lhs.len() + self.len_op().unwrap() + rhs.len()
                } else {
                    rhs.len()
                }
            }
            _ => self.lhs.len(),
        }
    }

    /// 右辺(行全体)のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_trailing_comment(&mut self, comment: Comment) {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            panic!(
                "set_trailing_comment:{:?} is not trailing comment!",
                comment
            );
        } else {
            let Comment { text, loc } = comment;
            // 1. 初めのハイフンを削除
            // 2. 空白、スペースなどを削除
            // 3. "--" を付与
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.trailing_comment = Some(trailing_comment);
            self.loc.append(loc);
        }
    }

    /// 左辺のtrailing_commentをセットする
    /// 複数行コメントを与えた場合パニックする
    pub(crate) fn set_lhs_trailing_comment(&mut self, comment: Comment) {
        if comment.is_multi_line_comment() {
            // 複数行コメント
            panic!(
                "set_lhs_trailing_comment:{:?} is not trailing comment!",
                comment
            );
        } else {
            // 行コメント
            let Comment { text, loc } = comment;
            let trailing_comment = format!("-- {}", text.trim_start_matches('-').trim_start());

            self.lhs_trailing_comment = Some(trailing_comment);
            self.loc.append(loc)
        }
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

    /// 複数行であるかどうかを返す
    fn is_multi_line(&self) -> bool {
        self.lhs.is_multi_line() || self.rhs.as_ref().map(Expr::is_multi_line).unwrap_or(false)
    }

    // 演算子までの長さを返す
    // 左辺の長さを返せばよい
    pub(crate) fn len_lhs(&self) -> usize {
        if self.lhs_trailing_comment.is_some() {
            // trailing commentが左辺にある場合、改行するため0
            0
        } else {
            self.lhs.len()
        }
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
        depth: usize,
        align_info: &AlignInfo,
        is_from_body: bool,
    ) -> Result<String, Error> {
        let mut result = String::new();

        let max_len_op = align_info.max_len_op;
        let max_len_to_op = align_info.max_len_to_op;
        let max_len_to_comment = align_info.max_len_to_comment;

        //左辺をrender
        let formatted = self.lhs.render()?;
        result.push_str(&formatted);

        let is_asterisk = matches!(self.lhs, Expr::Asterisk(_));

        // 演算子と右辺をrender
        match (&self.op, max_len_op, max_len_to_op) {
            (Some(op), Some(max_len_op), Some(max_len)) => {
                if let Some(comment_str) = &self.lhs_trailing_comment {
                    result.push('\t');
                    result.push_str(comment_str);
                    result.push('\n');

                    // インデントを挿入
                    result.extend(repeat_n('\t', depth));
                }

                let tab_num = max_len - self.len_lhs();
                result.extend(repeat_n('\t', tab_num));

                result.push('\t');

                // from句以外はopを挿入
                if !is_from_body {
                    result.push_str(op);
                    let tab_num = max_len_op - self.len_op().unwrap(); // self.op != Noneならlen_op != None
                    result.extend(repeat_n('\t', tab_num + 1));
                }

                //右辺をrender
                if let Some(rhs) = &self.rhs {
                    let formatted = rhs.render()?;
                    result.push_str(&formatted);
                }
            }
            // AS補完する場合
            (None, _, Some(max_len)) if COMPLEMENT_AS && self.is_alias && !is_asterisk => {
                let tab_num = max_len - self.lhs.len();
                result.extend(repeat_n('\t', tab_num));

                if !is_from_body {
                    result.push('\t');
                    result.push_str("AS");
                }
                // エイリアス補完はすべての演算子が"AS"であるかないため、すべての演算子の長さ(len_op())は等しい
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
            (_, _, _) => (),
        }

        // 末尾コメントをrender
        match (&self.trailing_comment, max_len_op, max_len_to_op) {
            // 末尾コメントが存在し、ほかのそろえる対象が存在する場合
            (Some(comment), Some(max_len_op), Some(max_len)) => {
                let tab_num = if let Some(rhs) = &self.rhs {
                    // 右辺がある場合は、コメントまでの最長の長さ - 右辺の長さ

                    // trailing_commentがある場合、max_len_to_commentは必ずSome(_)
                    max_len_to_comment.unwrap() - rhs.len()
                        + if rhs.is_multi_line() {
                            // 右辺が複数行である場合、最後の行に左辺と演算子はないため、その分タブで埋める
                            max_len + max_len_op
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
                    // コメントまでの最長 + 演算子の長さ + 左辺の最大長からの差分
                    max_len_to_comment.unwrap()
                        + (if is_from_body { 0 } else { max_len_op })
                        + max_len
                        - self.lhs.len()
                };

                result.extend(repeat_n('\t', tab_num));

                result.push('\t');
                result.push_str(comment);
            }
            // 末尾コメントが存在し、ほかにはそろえる対象が存在しない場合
            (Some(comment), _, None) => {
                // max_len_to_opがNoneであればそろえる対象はない
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
    trailing_comment: Option<Comment>, // Alignedが上位にない場合に末尾コメントを保持
}

impl PrimaryExpr {
    pub(crate) fn new(element: String, loc: Location) -> PrimaryExpr {
        let len = element.len() / TAB_SIZE + 1;
        PrimaryExpr {
            elements: vec![element],
            loc,
            len,
            head_comment: None,
            trailing_comment: None,
        }
    }

    pub(crate) fn loc(&self) -> Location {
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
            text: mut comment,
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

    // 末尾コメントをセットする
    // AlignedExprが上位にいない場合に呼び出される
    fn set_trailing_comment(&mut self, comment: Comment) {
        self.trailing_comment = Some(comment);
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
        let mut elements_str = self.elements.iter().map(|x| x.to_uppercase()).join("\t");

        // primaryに末尾コメントを追加する
        // 直後に改行が来ない場合にバグが生じる
        // TODO: 上位に、primaryの直後が改行でない場合、自動的に改行を挿入する処理を追加
        if let Some(comment) = &self.trailing_comment {
            elements_str.push('\t');
            elements_str.push_str(&comment.text);
        }

        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, elements_str)),
            None => Ok(elements_str),
        }
    }
}

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
    pub(crate) fn new(depth: usize, sep: &str) -> BooleanExpr {
        BooleanExpr {
            depth,
            default_separator: sep.to_string(),
            contents: vec![] as Vec<(String, AlignedExpr, Vec<Comment>)>,
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
                .set_trailing_comment(comment);
        }
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

        self.contents.push((sep, aligned, vec![]));
    }

    fn is_empty(&self) -> bool {
        self.contents.is_empty()
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
        for (sep, aligned, _) in other.contents {
            if is_first_content {
                self.add_expr_with_sep(aligned, self.default_separator.clone());
                is_first_content = false;
            } else {
                self.add_expr_with_sep(aligned, sep);
            }
        }
    }

    /// 比較演算子で揃えたものを返す
    pub(crate) fn render(&self) -> Result<String, Error> {
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

            let formatted = aligned.render(self.depth, &align_info, false)?;
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

    pub(crate) fn add_comment_to_child(&mut self, comment: Comment) {
        if self.expr.loc().is_same_line(&comment.loc()) {
            self.expr.add_comment_to_child(comment);
        } else {
            self.add_end_comment(comment);
        }
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

    pub(crate) fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        result.push_str("(\n");

        for comment in &self.start_comments {
            result.push_str(&comment.render(self.depth)?);
            result.push('\n');
        }

        let formatted = self.expr.render()?;

        result.push_str(&formatted);

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

    // 最後の式にコメントを追加する
    pub(crate) fn set_trailing_comment(&mut self, comment: Comment) {
        if let Some(else_clause) = self.else_clause.as_mut() {
            else_clause.add_comment_to_child(comment);
        } else if let Some(when_then_expr) = self.when_then_clause.last_mut() {
            when_then_expr.1.add_comment_to_child(comment);
        } else {
            // when_then/else が存在しない場合
            // つまり、CASEキーワードの直後にコメントが来た場合
            self.comments.push(comment);
        }
    }

    fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        // CASEキーワードの行のインデントは呼び出し側が行う
        result.push_str("CASE");
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
        result.push_str("END");

        Ok(result)
    }
}
