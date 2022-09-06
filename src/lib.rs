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
    let mut res = formatter.format_sql(root_node, src.as_ref());
    // eprintln!("{:#?}", res);

    // match res.render() {
    // Ok(res) => res,
    // Err(e) => panic!("{:?}", e),
    // }
    format!("{:#?}", res)
    // "".to_string()
}

#[derive(Debug)]
pub enum Error {
    ParseError,
}

#[derive(Debug, Clone)]
pub struct Line {
    elements: Vec<String>, // lifetimeの管理が面倒なのでStringに
    len: usize,
    len_to_as: Option<usize>, // AS までの距離
    len_to_op: Option<usize>, // 演算子までの距離(1行に一つ)
}

impl Line {
    pub fn new() -> Line {
        Line {
            elements: vec![] as Vec<String>,
            len: 0,
            len_to_as: None,
            len_to_op: None,
        }
    }

    pub fn contents(&self) -> &Vec<String> {
        &self.elements
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn len_to_as(&self) -> Option<usize> {
        self.len_to_as
    }

    pub fn len_to_op(&self) -> Option<usize> {
        self.len_to_op
    }

    /// 行の要素を足す(演算子はadd_operator()を使う)
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

    /// AS句を追加する
    pub fn add_as(&mut self, as_str: &str) {
        self.len_to_as = Some(self.len);
        self.add_element(as_str);
    }

    // 引数の文字列が比較演算子かどうかを判定する
    fn is_comp_op(op_str: &str) -> bool {
        match op_str {
            "<" | "<=" | "<>" | "!=" | "=" | ">" | ">=" | "~" | "!~" | "~*" | "!~*" => true,
            _ => false,
        }
    }

    /// 演算子を追加する
    pub fn add_op(&mut self, op_str: &str) {
        // 比較演算子のみをそろえる
        if Self::is_comp_op(op_str) {
            self.len_to_op = Some(self.len);
        }
        self.add_element(op_str);
    }

    // lineの結合
    pub fn append(&mut self, line: Line) {
        if let Some(len_to_as) = line.len_to_as() {
            // ASはlineに一つと仮定している
            self.len_to_as = Some(self.len + len_to_as);
        }

        if let Some(len_to_op) = line.len_to_op() {
            self.len_to_op = Some(self.len + len_to_op);
        }

        self.len += line.len();

        for content in (&line.contents()).into_iter() {
            self.elements.push(content.to_string());
        }
    }

    /// contentsを"\t"でjoinして返す
    pub fn to_string(&self) -> String {
        self.elements.join("\t")
    }
}

// #[derive(Debug, Clone)]
// pub enum Content {
//     SeparatedLines(SeparatedLines),
//     Line(Line),
// }

#[derive(Debug)]
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

    pub fn add_expr(&mut self, expr: AlignedExpr) {
        // len_to_opの更新
        if let Some(len) = expr.len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

        match self.loc {
            Some(mut range) => {
                range.end_point = expr.loc().end_point;
                self.loc = Some(range);
            }
            None => self.loc = Some(expr.loc()),
        };

        self.contents.push(expr);
    }

    /// AS句で揃えたものを返す
    pub fn render(&mut self) -> Result<String, Error> {
        todo!()
        // let mut result = String::new();

        // // 再帰的に再構成した木を見る

        // // for content in self.contents.clone() {
        // for i in 0..self.contents.len() {
        //     let content = self.contents.get(i).unwrap().clone();
        //     match content {
        //         Content::Line(line) => {
        //             //ネスト分だけ\tを挿入
        //             for current_depth in 0..self.depth {
        //                 // 1つ上のネストにsepを挿入
        //                 // ex)
        //                 //     depth = 2
        //                 //     sep = ","
        //                 //     の場合
        //                 //
        //                 //     "\t,\thoge"

        //                 if current_depth == self.depth - 1 {
        //                     if i != 0 {
        //                         result.push_str(self.separator.get(i).unwrap())
        //                     }
        //                 }
        //                 result.push_str("\t");
        //             }

        //             let mut current_len = 0;

        //             for j in 0..line.contents().len() {
        //                 let element = line.elements.get(j).unwrap();

        //                 // as, opなどまでの最大長とその行での長さを引数にとる
        //                 // 現在見ているcontentがas, opであれば、必要な数\tを挿入する
        //                 let mut insert_tab =
        //                     |max_len_to: Option<usize>, len_to: Option<usize>| -> () {
        //                         if let (Some(max_len_to), Some(len_to)) = (max_len_to, len_to) {
        //                             if current_len == len_to {
        //                                 let num_tab = (max_len_to / TAB_SIZE) - (len_to / TAB_SIZE);
        //                                 for _ in 0..num_tab {
        //                                     result.push_str("\t");
        //                                 }
        //                             };
        //                         };
        //                     };

        //                 // ASの位置揃え
        //                 insert_tab(self.max_len_to_as, line.len_to_as());
        //                 // OPの位置揃え
        //                 insert_tab(self.max_len_to_op, line.len_to_op());

        //                 result.push_str(element);

        //                 //最後のelement以外は"\t"を挿入
        //                 if j != line.contents().len() - 1 {
        //                     result.push('\t');
        //                 }

        //                 // element.len()より大きく、かつTAB_SIZEの倍数のうち最小のものを足す
        //                 current_len += TAB_SIZE * (element.len() / TAB_SIZE + 1);
        //             }

        //             result.push_str("\n");
        //         }
        //         Content::SeparatedLines(mut sl) => {
        //             // 再帰的にrender()を呼び、結果をresultに格納
        //             let sl_res = sl.render();

        //             match sl_res {
        //                 Ok(res) => result.push_str(&res),
        //                 Err(e) => panic!("{:?}", e),
        //             }
        //         }
        //     }
        // }
        // Ok(result)
    }
}

#[derive(Debug)]
pub struct Statement {
    clauses: Vec<Clause>,
    loc: Option<Range>,
}

impl Statement {
    pub fn new() -> Statement {
        Statement {
            clauses: vec![] as Vec<Clause>,
            loc: None,
        }
    }

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
}

#[derive(Debug)]
pub struct Clause {
    keyword: String, // e.g., SELECT, FROM
    body: Option<SeparatedLines>,
    loc: Range,
}

impl Clause {
    pub fn new(keyword: String, loc: Range) -> Clause {
        Clause {
            keyword,
            body: None,
            loc,
        }
    }

    pub fn loc(&self) -> Range {
        self.loc
    }

    // bodyをセットする
    pub fn set_body(&mut self, body: SeparatedLines) {
        self.loc.end_point = body.loc().unwrap().end_point;
        self.body = Some(body);
    }
}

// enum Expr {
//     Aligned(Box<AlignedExpr>),
//     Primary(Box<PrimaryExpr>),
//     Boolean(Box<SeparatedLines>),
// }

pub trait Expr {
    fn loc(&self) -> Range;
    fn len(&self) -> usize;

    fn to_primary(&self) -> Option<PrimaryExpr>;
}

use std::fmt::Debug;

impl Debug for dyn Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hogehoge")
    }
}

// 次を入れるとエラーになる
#[derive(Debug)]
pub struct AlignedExpr {
    lhs: Box<dyn Expr>,
    rhs: Option<Box<dyn Expr>>,
    op: Option<String>,
    loc: Range,
    tail_comment: Option<String>,
}

impl Expr for AlignedExpr {
    fn loc(&self) -> Range {
        self.loc
    }

    fn len(&self) -> usize {
        todo!()
    }

    fn to_primary(&self) -> Option<PrimaryExpr> {
        None
    }
}

impl AlignedExpr {
    pub fn new(lhs: Box<dyn Expr>, loc: Range) -> AlignedExpr {
        AlignedExpr {
            lhs,
            rhs: None,
            op: None,
            loc,
            tail_comment: None,
        }
    }

    pub fn add_rhs(&mut self, op: String, rhs: Box<dyn Expr>) {
        self.loc.end_point = rhs.loc().end_point;
        self.op = Some(op);
        self.rhs = Some(rhs);
    }

    pub fn len_to_op(&self) -> Option<usize> {
        match self.op {
            Some(_) => Some(self.lhs.len()),
            None => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PrimaryExpr {
    elements: Vec<String>,
    loc: Range,
    len: usize,
    // head_comment: Option<String>,
}

impl Expr for PrimaryExpr {
    fn loc(&self) -> Range {
        self.loc
    }

    fn len(&self) -> usize {
        self.len
    }

    fn to_primary(&self) -> Option<PrimaryExpr> {
        Some(self.clone())
    }
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

    pub fn element(&self) -> &Vec<String> {
        &self.elements
    }

    pub fn len(&self) -> usize {
        self.len
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

    // lineの結合
    pub fn append(&mut self, primary: PrimaryExpr) {
        self.elements.append(&mut primary.element().clone())
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
    fn goto_not_comment_next_sibiling_for_line(
        &mut self,
        line: &mut Line,
        cursor: &mut TreeCursor,
        src: &str,
    ) -> bool {
        //兄弟ノードがない場合
        if !cursor.goto_next_sibling() {
            return false;
        }

        //コメントノードであればbufに追記していく
        while cursor.node().kind() == "comment" {
            let comment_node = cursor.node();
            line.add_element(comment_node.utf8_text(src.as_bytes()).unwrap());

            if !cursor.goto_next_sibling() {
                return false;
            }
        }

        return true;
    }

    fn format_source(&mut self, node: Node, src: &str) -> Statement {
        // source_file -> _statement*

        // let mut result = SeparatedLines::new(self.state.depth, "");

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
            let stmt = self.format_select_stmt(stmt_node, src);
            return stmt;
            //select_statement以外も追加した場合この部分は削除
            // self.goto_not_comment_next_sibiling(buf, &mut cursor, src);
        }

        // result
        todo!()
    }

    // SELECT文
    fn format_select_stmt(&mut self, node: Node, src: &str) -> Statement {
        /*
            _select_statement ->
                select_clause
                from_clause?
                << 未対応!! join_clause* >>
                where_clause?
                << 未対応!! ... >>
        */

        let mut statement = Statement::new();

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            // select_clauseを指す
            let select_clause_node = cursor.node();

            statement.add_clause(self.format_select_clause(select_clause_node, src));
        }

        loop {
            // 次の兄弟へ移動
            // if !self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            //     break; // 子供がいなくなったら脱出
            // }
            if !cursor.goto_next_sibling() {
                break;
            }

            let clause_node = cursor.node();
            // println!("{}", clause_node.kind());

            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // FROMを指している

                        let mut clause = Clause::new("FROM".to_string(), cursor.node().range());

                        self.nest();
                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",");

                        // commaSep
                        // selectのときと同じであるため、統合したい
                        // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
                        if cursor.goto_next_sibling() {
                            let expr_node = cursor.node();

                            separated_lines.add_expr(self.format_aliasable_expr(expr_node, src));

                            // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
                            while cursor.goto_next_sibling() {
                                let child_node = cursor.node();

                                match child_node.kind() {
                                    "," => {
                                        continue;
                                    }
                                    _ => {
                                        separated_lines
                                            .add_expr(self.format_aliasable_expr(child_node, src));
                                    }
                                };
                            }
                        }

                        // result.add_content(Content::SeparatedLines(separated_lines));
                        // buf.push_str(separated_lines.render().as_ref());

                        cursor.goto_parent();
                        self.unnest();

                        clause.set_body(separated_lines);

                        statement.add_clause(clause);
                    }
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    if cursor.goto_first_child() {
                        // let mut line = Line::new();
                        // line.add_element("WHERE");

                        let mut clause = Clause::new("WHERE".to_string(), cursor.node().range());

                        self.nest();
                        let mut separated_lines = SeparatedLines::new(self.state.depth, "");

                        // result.add_content(Content::Line(line));

                        // self.goto_not_comment_next_sibiling(buf, &mut cursor, src);
                        cursor.goto_next_sibling();

                        //expr
                        let expr_node = cursor.node();
                        let expr = self.format_expr(expr_node, src);
                        let loc = expr.loc();
                        let aligned = AlignedExpr::new(expr, loc);
                        separated_lines.add_expr(aligned);
                        // buf.push_str(bool_expr.as_str());

                        // eprintln!("{:#?}", separated_lines);
                        // result.add_content(Content::SeparatedLines(separated_lines));
                        self.unnest();
                        cursor.goto_parent();

                        clause.set_body(separated_lines);

                        statement.add_clause(clause);
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
                (   _aliasable_expression
                    << 未対応!! ("INTO" identifier) >>
                    (   ","
                        (   _aliasable_expression
                            << 未対応!! ("INTO" identifier) >>
                        )
                    )
                )?
        */
        let mut cursor = node.walk(); // select_clauseノードのはず

        let mut clause = Clause::new("SELECT".to_string(), node.range());

        if cursor.goto_first_child() {
            // SELECTに移動
            // select_clauseの最初の子供は必ず"SELECT"であるはず
            // println!("expect SELECT, acutal {}", cursor.node().kind());

            // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            cursor.goto_next_sibling();
            // select_caluse_body
            // println!("expect select_clause_body, actual {}", cursor.node().kind());

            // select_clause_bodyをカーソルが指している
            let body = self.format_select_clause_body(cursor.node(), src);
            clause.set_body(body);
        }

        clause
    }

    fn format_select_clause_body(&mut self, node: Node, src: &str) -> SeparatedLines {
        let mut cursor = node.walk();
        // println!("select_clause_body, {}", cursor.node().kind());

        cursor.goto_first_child();

        // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

        // 最初のノードは_aliasable_expressionのはず
        let expr_node = cursor.node();

        self.nest();
        let mut sepapated_lines = SeparatedLines::new(self.state.depth, ",");

        let content = self.format_aliasable_expr(expr_node, src);
        sepapated_lines.add_expr(content);

        self.unnest();

        // (',' _aliasable_expression)*
        // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
        while cursor.goto_next_sibling() {
            let child_node = cursor.node();
            match child_node.kind() {
                "," => {
                    continue;
                }
                _ => {
                    let aligned = self.format_aliasable_expr(child_node, src);
                    sepapated_lines.add_expr(aligned);
                }
            }
        }

        sepapated_lines
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
                cursor.goto_first_child();

                // _expression
                let lhs_expr = self.format_expr(cursor.node(), src);
                let loc = lhs_expr.loc();

                let mut aligned = AlignedExpr::new(lhs_expr, loc);

                // ("AS"? identifier)?
                if cursor.goto_next_sibling() && cursor.node().kind() == "AS" {
                    // "AS"?

                    //左辺に移動
                    cursor.goto_next_sibling();

                    // identifier
                    if cursor.node().kind() == "identifier" {
                        let rhs = cursor.node().utf8_text(src.as_bytes()).unwrap();
                        aligned.add_rhs(
                            "AS".to_string(),
                            Box::new(PrimaryExpr::new(rhs.to_string(), cursor.node().range())),
                        );
                    }
                }
                aligned
            }
            _ => {
                // _expression

                let mut cursor = node.walk();
                let expr = self.format_expr(cursor.node(), src);
                let loc = expr.loc();

                let mut aligned = AlignedExpr::new(expr, loc);

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
    fn format_expr(&mut self, node: Node, src: &str) -> Box<dyn Expr> {
        match node.kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                let mut cursor = node.walk();
                let range = node.range();

                cursor.goto_first_child();

                let mut result = String::new();

                let id_node = cursor.node();
                result.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                // while self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src) {
                while cursor.goto_next_sibling() {
                    match cursor.node().kind() {
                        "." => result.push_str("."),
                        _ => result.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }

                let primary = PrimaryExpr::new(result, range);

                Box::new(primary)
            }
            "binary_expression" => {
                let mut line = Line::new();

                let mut cursor = node.walk();
                cursor.goto_first_child();

                // 左辺
                let lhs_node = cursor.node();
                let mut lhs_expr = self.format_expr(lhs_node, src);

                // match lhs_line {
                //     Content::Line(ln) => {
                //         line.append(ln);
                //     }
                //     Content::SeparatedLines(_) => {
                //         //右辺が複数行の場合は未対応
                //     }
                // }

                // 演算子
                // self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                cursor.goto_next_sibling();
                let op_node = cursor.node();
                let op_str = op_node.utf8_text(src.as_ref()).unwrap();

                // 右辺
                cursor.goto_next_sibling();
                let rhs_node = cursor.node();
                let rhs_expr = self.format_expr(rhs_node, src);

                if Self::is_comp_op(op_str) {
                    // 比較演算子 -> AlignedExpr
                    let loc = lhs_expr.loc();
                    let mut aligned = AlignedExpr::new(lhs_expr, loc);
                    aligned.add_rhs(op_str.to_string(), rhs_expr);

                    Box::new(aligned)
                } else {
                    // 比較演算子でない -> PrimaryExpr
                    // e.g.,) 1 + 1
                    let mut lhs_expr = lhs_expr.to_primary().unwrap();
                    lhs_expr.add_element(op_str);
                    lhs_expr.append(rhs_expr.to_primary().unwrap());

                    Box::new(lhs_expr)
                }

                // line.add_op(op_node.utf8_text(src.as_bytes()).unwrap());

                // match expr_line {
                //     Content::Line(ln) => {
                //         line.append(ln);
                //     }
                //     Content::SeparatedLines(_) => {
                //         //右辺が複数行の場合は未対応
                //     }
                // }
                // res = Content::Line(line);
            }
            "boolean_expression" => {
                todo!()
                // res = self.format_bool_expr(node, src);
            }
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let mut primary = PrimaryExpr::new(
                    node.utf8_text(src.as_bytes()).unwrap().to_string(),
                    node.range(),
                );
                Box::new(primary)
            }
            _ => {
                eprintln!("format_expr(): unknown node ({}).", node.kind());
                todo!()
                // let mut line = Line::new();
                // line.add_element(self.format_straightforward(node, src).as_ref());
                // res = Content::Line(line);
            }
        }
    }

    // fn format_bool_expr(&mut self, node: Node, src: &str) -> Content {
    /*
    boolean_expression: $ =>
        choice(
        prec.left(PREC.unary, seq(kw("NOT"), $._expression)),
        prec.left(PREC.and, seq($._expression, kw("AND"), $._expression)),
        prec.left(PREC.or, seq($._expression, kw("OR"), $._expression)),
    ),
     */

    // let mut sep_lines = SeparatedLines::new(self.state.depth, "");

    // let mut cursor = node.walk();

    // cursor.goto_first_child();

    // if cursor.node().kind() == "NOT" {
    // 未対応
    // } else {
    //         let left = self.format_expr(cursor.node(), src);
    //         cursor.goto_next_sibling();
    //         let sep = cursor.node().kind();
    //         cursor.goto_next_sibling();
    //         let right = self.format_expr(cursor.node(), src);

    //         sep_lines.set_separator(sep);
    //         sep_lines.merge(left);
    //         sep_lines.merge(right);
    //     }
    //     Content::SeparatedLines(sep_lines)
    // }

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
