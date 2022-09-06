use tree_sitter::{Node, Range, TreeCursor};

const TAB_SIZE: usize = 4;

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
    let mut formatter = Formatter::new();

    // formatを行い、バッファに結果を格納
    let res = formatter.format_sql(root_node, src.as_ref());
    eprintln!("{:#?}", res);

    match res.render() {
        Ok(res) => res,
        Err(e) => panic!("{:?}", e),
    }
}

#[derive(Debug)]
pub enum Error {
    ParseError,
}

// 句の本体にあたる部分である、あるseparatorで区切られた式の集まり
#[derive(Debug, Clone)]
pub struct SeparatedLines {
    depth: usize,               // インデントの深さ
    separator: String,          // セパレータ(e.g., ',', AND)
    contents: Vec<AlignedExpr>, // 各行の情報
    loc: Option<Range>,
    max_len_to_op: Option<usize>, // 演算子までの最長の長さ(1行に一つと仮定)
}

// BooleanExpr: Expr

impl SeparatedLines {
    pub fn new(depth: usize, sep: &str) -> SeparatedLines {
        SeparatedLines {
            depth,
            separator: sep.to_string(),
            contents: vec![] as Vec<AlignedExpr>,
            loc: None,
            max_len_to_op: None,
        }
    }

    pub fn loc(&self) -> Option<Range> {
        self.loc
    }

    pub fn max_len_to_op(&self) -> Option<usize> {
        self.max_len_to_op
    }

    // 式を追加する
    pub fn add_expr(&mut self, expr: AlignedExpr) {
        // len_to_opの更新
        if let Some(len) = expr.len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

        // locationの更新
        match self.loc {
            Some(mut range) => {
                range.end_point = expr.loc().end_point;
                self.loc = Some(range);
            }
            None => self.loc = Some(expr.loc()),
        };

        self.contents.push(expr);
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        let last_idx = self.contents.len() - 1;
        self.contents[last_idx].set_tail_comment(comment);
    }

    /// AS句で揃えたものを返す
    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        let mut is_first_line = true;

        for aligned in (&self.contents).into_iter() {
            // ネストは後で

            (0..self.depth)
                .into_iter()
                .for_each(|_| result.push_str("\t"));

            if is_first_line {
                is_first_line = false;
                result.push_str("\t")
            } else {
                result.push_str(&self.separator);
                result.push_str("\t");
            }

            // alignedに演算子までの最長の長さを与えてフォーマット済みの文字列をもらう
            match aligned.render(self.max_len_to_op) {
                Ok(formatted) => {
                    result.push_str(&formatted);
                    result.push_str("\n")
                }
                Err(e) => return Err(e),
            };
        }

        Ok(result)
    }
}

// *_statementに対応した構造体
#[derive(Debug, Clone)]
pub struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Range>,
    depth: usize,
}

impl Statement {
    pub fn new(depth: usize) -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
            depth,
        }
    }

    pub fn loc(&self) -> Option<Range> {
        self.loc
    }

    // 文に句を追加する
    pub fn add_clause(&mut self, clause: Clause) {
        match self.loc {
            Some(mut loc) => {
                loc.end_point = clause.loc().end_point;
                self.loc = Some(loc)
            }
            None => {
                self.loc = Some(clause.loc());
            }
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

        let mut result = String::new();
        for i in 0..self.clauses.len() {
            // 後でイテレータで書き直す
            let clause = self.clauses.get(i).unwrap();
            match clause.render() {
                Ok(formatted_clause) => result.push_str(&formatted_clause),
                Err(_) => return Err(Error::ParseError),
            }
        }

        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct Comment {
    comment: String,
    loc: Range,
}

impl Comment {
    pub fn new(comment: String, loc: Range) -> Comment {
        Comment { comment, loc }
    }
}

#[derive(Debug, Clone)]
pub enum Body {
    SepLines(SeparatedLines),
    BooleanExpr(BooleanExpr),
}

impl Body {
    pub fn loc(&self) -> Option<Range> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.loc(),
            Body::BooleanExpr(bool_expr) => bool_expr.loc(),
        }
    }

    pub fn render(&self) -> Result<String, Error> {
        match self {
            Body::SepLines(sep_lines) => sep_lines.render(),
            Body::BooleanExpr(bool_expr) => bool_expr.render(),
        }
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Body::SepLines(sep_lines) => sep_lines.add_comment_to_child(comment),
            Body::BooleanExpr(bool_expr) => bool_expr.add_comment_to_child(comment),
        }
    }
}

// 句に対応した構造体
#[derive(Debug, Clone)]
pub struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<Body>,
    loc: Range,
    depth: usize,
}

impl Clause {
    pub fn new(keyword: String, loc: Range, depth: usize) -> Clause {
        Clause {
            keyword,
            body: None,
            loc,
            depth,
        }
    }

    pub fn loc(&self) -> Range {
        self.loc
    }

    // bodyをセットする
    pub fn set_body(&mut self, body: Body) {
        self.loc.end_point = body.loc().unwrap().end_point;
        self.body = Some(body);
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        match &mut self.body {
            Some(body) => body.add_comment_to_child(comment),
            _ => (),
        }
    }

    pub fn render(&self) -> Result<String, Error> {
        // kw
        // body...
        let mut result = String::new();

        (0..self.depth)
            .into_iter()
            .for_each(|_| result.push_str("\t"));

        result.push_str(&self.keyword);

        match &self.body {
            Some(sl) => match sl.render() {
                Ok(formatted_body) => {
                    result.push_str("\n");
                    result.push_str(&formatted_body);
                }
                Err(e) => return Err(e),
            },
            None => (),
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
}

impl Expr {
    fn loc(&self) -> Range {
        match self {
            Expr::Aligned(aligned) => aligned.loc(),
            Expr::Primary(primary) => primary.loc(),
            Expr::Boolean(sep_lines) => sep_lines.loc().unwrap(),
            Expr::SelectSub(select_sub) => select_sub.loc(),
        }
    }

    fn render(&self) -> Result<String, Error> {
        match self {
            Expr::Aligned(aligned) => {
                todo!();
                // aligned.render();
            }
            Expr::Primary(primary) => primary.render(),
            Expr::Boolean(boolean) => boolean.render(),
            Expr::SelectSub(select_sub) => select_sub.render(),
        }
    }

    // 最後の行の長さをタブ文字換算した結果を返す
    fn len(&self) -> usize {
        match self {
            Expr::Primary(primary) => primary.len(),
            Expr::SelectSub(_) => TAB_SIZE, // 必ずかっこなので、TAB_SIZE
            _ => todo!(),
        }
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        match self {
            Expr::Aligned(aligned) => aligned.set_tail_comment(comment),
            Expr::Primary(primary) => (),
            Expr::Boolean(boolean) => boolean.add_comment_to_child(comment),
        }
    }
}

// 次を入れるとエラーになる
#[derive(Debug, Clone)]
pub struct AlignedExpr {
    lhs: Expr,
    rhs: Option<Expr>,
    op: Option<String>,
    loc: Range,
    tail_comment: Option<String>, // 行末コメント
}

impl AlignedExpr {
    pub fn new(lhs: Expr, loc: Range) -> AlignedExpr {
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            tail_comment: None,
        }
    }

    fn loc(&self) -> Range {
        self.loc
    }

    pub fn set_tail_comment(&mut self, comment: Comment) {
        if let Comment { comment, loc } = comment {
            self.tail_comment = Some(comment.clone());
            self.loc.end_point = loc.end_point;
        }
    }

    // 演算子と右辺の式を追加する
    pub fn add_rhs(&mut self, op: String, rhs: Expr) {
        self.loc.end_point = rhs.loc().end_point;
        self.op = Some(op);
        self.rhs = Some(rhs);
    }

    // 演算子までの長さを返す
    pub fn len_to_op(&self) -> Option<usize> {
        // 左辺の長さを返せばよい
        match self.op {
            Some(_) => Some(self.lhs.len()),
            None => None,
        }
    }

    // 演算子までの長さを与え、演算子の前にtab文字を挿入した文字列を返す
    pub fn render(&self, max_len_to_op: Option<usize>) -> Result<String, Error> {
        let mut result = String::new();

        //左辺をrender
        match self.lhs.render() {
            Ok(formatted) => result.push_str(&formatted),
            Err(e) => return Err(e),
        };

        // 演算子等辺をrender
        match (&self.op, max_len_to_op) {
            (Some(op), Some(max_len)) => {
                let tab_num = (max_len - self.lhs.len()) / TAB_SIZE;

                (0..tab_num).into_iter().for_each(|_| result.push_str("\t"));

                result.push_str("\t");
                result.push_str(&op);
                result.push_str("\t");

                //右辺をrender
                match &self.rhs {
                    Some(rhs) => {
                        let formatted = rhs.render().unwrap();
                        result.push_str(&formatted);
                    }
                    _ => (),
                }
            }
            (_, _) => (),
        }

        //とりあえずコメントそのまま出力
        match &self.tail_comment {
            Some(comment) => {
                result.push_str("\t");
                result.push_str(comment);
                eprintln!("{:#?}", comment);
            }
            None => (),
        }

        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct PrimaryExpr {
    elements: Vec<String>,
    loc: Range,
    len: usize,
    // head_comment: Option<String>,
}

impl PrimaryExpr {
    pub fn new(element: String, loc: Range) -> PrimaryExpr {
        let len = TAB_SIZE * (element.len() / TAB_SIZE + 1);
        PrimaryExpr {
            elements: vec![element],
            loc,
            len,
        }
    }

    fn loc(&self) -> Range {
        self.loc
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn elements(&self) -> &Vec<String> {
        &self.elements
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
        self.elements.append(&mut primary.elements().clone())
    }

    pub fn render(&self) -> Result<String, Error> {
        let upper_elements: Vec<String> = self.elements.iter().map(|x| x.to_uppercase()).collect();
        Ok(upper_elements.join("\t"))
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
    loc: Option<Range>,
    max_len_to_op: Option<usize>, // 演算子までの最長の長さ(1行に一つと仮定)
}

impl BooleanExpr {
    pub fn new(depth: usize, sep: &str) -> BooleanExpr {
        BooleanExpr {
            depth,
            default_separator: sep.to_string(),
            contents: vec![] as Vec<ContentWithSep>,
            loc: None,
            max_len_to_op: None,
        }
    }

    pub fn loc(&self) -> Option<Range> {
        self.loc
    }

    pub fn max_len_to_op(&self) -> Option<usize> {
        self.max_len_to_op
    }

    pub fn set_default_separator(&mut self, sep: String) {
        self.default_separator = sep;
    }

    pub fn add_comment_to_child(&mut self, comment: Comment) {
        let last_idx = self.contents.len() - 1;
        self.contents[last_idx].content.set_tail_comment(comment);
    }

    pub fn add_expr_with_sep(&mut self, expr: AlignedExpr, sep: String) {
        // len_to_opの更新
        if let Some(len) = expr.len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

        // locationの更新
        match self.loc {
            Some(mut range) => {
                range.end_point = expr.loc().end_point;
                self.loc = Some(range);
            }
            None => self.loc = Some(expr.loc()),
        };

        self.contents.push(ContentWithSep {
            separator: sep,
            content: expr,
        });
    }

    pub fn add_expr(&mut self, expr: AlignedExpr) {
        self.add_expr_with_sep(expr, self.default_separator.clone());
    }

    pub fn merge(&mut self, other: BooleanExpr) {
        // len_to_opの更新
        if let Some(len) = other.max_len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

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

    /// AS句で揃えたものを返す
    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        // 再帰的に再構成した木を見る

        let mut is_first_line = true;

        for ContentWithSep { separator, content } in (&self.contents).into_iter() {
            (0..self.depth)
                .into_iter()
                .for_each(|_| result.push_str("\t"));

            if is_first_line {
                is_first_line = false;
                result.push_str("\t")
            } else {
                result.push_str(&separator);
                result.push_str("\t");
            }

            match content.render(self.max_len_to_op) {
                Ok(formatted) => {
                    result.push_str(&formatted);
                    result.push_str("\n")
                }
                Err(e) => return Err(e),
            };
        }

        Ok(result)
    }
}

// SELECTサブクエリに対応する構造体
#[derive(Debug, Clone)]
pub struct SelectSubExpr {
    depth: usize,
    stmt: Statement,
    loc: Range,
}

impl SelectSubExpr {
    pub fn new(stmt: Statement, loc: Range, depth: usize) -> SelectSubExpr {
        SelectSubExpr { depth, stmt, loc }
    }

    pub fn loc(&self) -> Range {
        self.loc
    }

    pub fn render(&self) -> Result<String, Error> {
        let mut result = String::new();

        result.push_str("(\n");

        match self.stmt.render() {
            Ok(formatted) => {
                result.push_str(&formatted);

                (0..self.depth)
                    .into_iter()
                    .for_each(|_| result.push_str("\t"));

                result.push_str(")");
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }
}

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub depth: usize,
}

pub struct Formatter {
    state: FormatterState,
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

    // goto_next_sibiling()をコメントの処理を行うように拡張したもの
    // fn goto_not_comment_next_sibiling_for_line(
    //     &mut self,
    //     line: &mut Line,
    //     cursor: &mut TreeCursor,
    //     src: &str,
    // ) -> bool {
    //     //兄弟ノードがない場合
    //     if !cursor.goto_next_sibling() {
    //         return false;
    //     }

    //     //コメントノードであればbufに追記していく
    //     while cursor.node().kind() == "comment" {
    //         let comment_node = cursor.node();
    //         line.add_element(comment_node.utf8_text(src.as_bytes()).unwrap());

    //         if !cursor.goto_next_sibling() {
    //             return false;
    //         }
    //     }

    //     return true;
    // }

    fn format_source(&mut self, node: Node, src: &str) -> Statement {
        // source_file -> _statement*

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            //コメントノードであればbufに追記していく
            // while cursor.node().kind() == "comment" {
            //     let comment_node = cursor.node();
            //     buf.push_str(comment_node.utf8_text(src.as_bytes()).unwrap());
            //     buf.push_str("\n");
            //     cursor.goto_next_sibling();
            // }

            let stmt_node = cursor.node();

            // 現状はselect_statementのみ
            // 文が増えたらマッチ式で分岐させる
            let mut stmt = self.format_select_stmt(stmt_node, src);

            if cursor.goto_next_sibling() && cursor.node().kind() == "comment" {
                let comment_loc = cursor.node().range();

                //同じ行の場合
                if comment_loc.start_point.row == stmt.loc().unwrap().end_point.row {
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

        let mut statement = Statement::new(self.state.depth);

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
                        let mut clause =
                            Clause::new("FROM".to_string(), from_node.range(), self.state.depth);

                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",");

                        // commaSep
                        if cursor.goto_next_sibling() {
                            // cursor -> _aliasable_expression
                            let expr_node = cursor.node();

                            separated_lines.add_expr(self.format_aliasable_expr(expr_node, src));

                            while cursor.goto_next_sibling() {
                                // cursor -> , または cursor -> _aliasable_expression
                                let child_node = cursor.node();

                                match child_node.kind() {
                                    "," => {
                                        continue;
                                    }
                                    "comment" => {
                                        let comment_loc = child_node.range();

                                        //同じ行の場合
                                        if comment_loc.start_point.row
                                            == separated_lines.loc().unwrap().end_point.row
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
                                        separated_lines
                                            .add_expr(self.format_aliasable_expr(child_node, src));
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
                        let mut clause =
                            Clause::new("WHERE".to_string(), where_node.range(), self.state.depth);

                        let mut body: Body;
                        // let mut separated_lines = SeparatedLines::new(self.state.depth, "");

                        // self.goto_not_comment_next_sibiling(buf, &mut cursor, src);
                        cursor.goto_next_sibling();
                        // cursor -> _expression

                        let expr_node = cursor.node();
                        let expr = self.format_expr(expr_node, src);
                        let expr_loc = expr.loc();

                        match expr {
                            Expr::Aligned(aligned) => {
                                let mut separated_lines = SeparatedLines::new(self.state.depth, "");
                                separated_lines.add_expr(*aligned);
                                body = Body::SepLines(separated_lines);
                            }
                            Expr::Primary(_) => {
                                todo!();
                                // let mut separated_lines = SeparatedLines::new(self.state.depth, "");
                                // separated_lines.add_expr(AlignedExpr::new(expr, expr_loc));
                                // body = Body::SepLines(separated_lines);
                            }
                            Expr::Boolean(boolean) => body = Body::BooleanExpr(*boolean),
                            Expr::SelectSub(select_sub) => todo!(),
                        }

                        cursor.goto_parent();

                        clause.set_body(body);

                        statement.add_clause(clause);
                    }
                }
                "comment" => {
                    let comment_loc = clause_node.range();

                    //同じ行の場合
                    if comment_loc.start_point.row == statement.loc().unwrap().end_point.row {
                        statement.add_comment_to_child(Comment::new(
                            clause_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                            clause_node.range(),
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

        let mut clause = Clause::new("SELECT".to_string(), node.range(), self.state.depth);

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

        let mut separated_lines = SeparatedLines::new(self.state.depth, ",");

        let aligned = self.format_aliasable_expr(expr_node, src);
        separated_lines.add_expr(aligned);

        // (',' _aliasable_expression)*
        // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
        while cursor.goto_next_sibling() {
            // cursor -> , または cursor -> _aliasable_expression
            let child_node = cursor.node();
            match child_node.kind() {
                "," => {
                    continue;
                }
                "comment" => {
                    separated_lines.add_comment_to_child(Comment::new(
                        child_node.utf8_text(src.as_bytes()).unwrap().to_string(),
                        child_node.range(),
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

                let mut aligned = AlignedExpr::new(lhs_expr, lhs_expr_loc);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() && cursor.node().kind() == "AS" {
                    // cursor -> AS ("AS"が省略されることは現状考慮していない)

                    //左辺に移動
                    cursor.goto_next_sibling();
                    // cursor -> identifier

                    // identifier
                    if cursor.node().kind() == "identifier" {
                        let rhs = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        let rhs_expr = PrimaryExpr::new(rhs.to_string(), cursor.node().range());
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

                let aligned = AlignedExpr::new(expr, expr_loc);

                aligned
            }
        }
    }

    // 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        match op_str {
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*" => true,
            _ => false,
        }
    }

    // 式
    fn format_expr(&mut self, node: Node, src: &str) -> Expr {
        match node.kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                let mut cursor = node.walk();
                // cursor -> dotted_name

                let range = node.range();

                cursor.goto_first_child();
                // cursor -> identifier

                let mut result = String::new();

                let id_node = cursor.node();
                result.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                // while self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src) {
                while cursor.goto_next_sibling() {
                    // cursor -> . または cursor -> identifier
                    match cursor.node().kind() {
                        "." => result.push_str("."),
                        _ => result.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(result, range);

                Expr::Primary(Box::new(primary))
            }
            "binary_expression" => {
                let mut cursor = node.walk();
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

                // 右辺
                let rhs_node = cursor.node();
                let rhs_expr = self.format_expr(rhs_node, src);

                if Self::is_comp_op(op_str) {
                    // 比較演算子 -> AlignedExpr
                    let lhs_loc = lhs_expr.loc();
                    let mut aligned = AlignedExpr::new(lhs_expr, lhs_loc);
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
                    node.range(),
                );
                Expr::Primary(Box::new(primary))
            }
            "select_subexpression" => {
                self.nest();
                let select_subexpr = self.format_select_subexpr(node, src);
                self.unnest();
                Expr::SelectSub(Box::new(select_subexpr))
            }
            _ => {
                eprintln!("format_expr(): unimplemented expression {}", node.kind());
                todo!()
                // let mut line = Line::new();
                // line.add_element(self.format_straightforward(node, src).as_ref());
                // res = Content::Line(line);
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
            // 未対応
        } else {
            let left = self.format_expr(cursor.node(), src);
            match left {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
            }

            cursor.goto_next_sibling();

            let sep = cursor.node().kind();
            boolean_expr.set_default_separator(sep.to_string());

            cursor.goto_next_sibling();
            let right = self.format_expr(cursor.node(), src);

            match right {
                Expr::Aligned(aligned) => boolean_expr.add_expr(*aligned),
                Expr::Primary(_) => todo!(),
                Expr::Boolean(boolean) => boolean_expr.merge(*boolean),
                Expr::SelectSub(_) => todo!(),
            }
        }
        Expr::Boolean(Box::new(boolean_expr))
    }

    fn format_select_subexpr(&mut self, node: Node, src: &str) -> SelectSubExpr {
        // select_subexpression -> "(" select_statement ")"

        let loc = node.range();

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

    // 未対応の構文をそのまま表示する(dfs)
    fn format_straightforward(&mut self, node: Node, src: &str) -> String {
        let mut result = String::new();

        // 葉である場合resultに追加
        if node.child_count() <= 0 {
            result.push_str(
                node.utf8_text(src.as_bytes())
                    .unwrap()
                    .to_ascii_uppercase()
                    .as_ref(),
            );
            result.push_str("\n");
            return result;
        }

        // 葉でない場合
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                result.push_str(self.format_straightforward(cursor.node(), src).as_ref());
                if cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        result
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

    dfs(root_node, 0);
    eprintln!("");
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
