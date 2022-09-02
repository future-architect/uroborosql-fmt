use tree_sitter::{Node, TreeCursor};

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
    println!("{:#?}", res);

    res.render()
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

#[derive(Debug, Clone)]
pub enum Content {
    SeparatedLines(SeparatedLines),
    Line(Line),
}

#[derive(Debug, Clone)]
pub struct SeparatedLines {
    depth: usize,                 // インデントの深さ
    separetor: String,            // セパレータ(e.g., ',', AND)
    contents: Vec<Content>,       // 各行の情報
    max_len_to_as: Option<usize>, // ASまでの最長の長さ
    max_len_to_op: Option<usize>, // 演算子までの最長の長さ(1行に一つと仮定)
}

impl SeparatedLines {
    pub fn new(depth: usize, sep: &str) -> SeparatedLines {
        SeparatedLines {
            depth,
            separetor: sep.to_string(),
            contents: vec![] as Vec<Content>,
            max_len_to_as: None,
            max_len_to_op: None,
        }
    }

    pub fn set_separetor(&mut self, sep: &str) {
        self.separetor = sep.to_string();
    }

    /// Contentを追加
    pub fn add_content(&mut self, content: Content) {
        match content {
            Content::Line(line) => {
                // len_to_asの更新
                if let Some(len) = line.len_to_as() {
                    self.max_len_to_as = match self.max_len_to_as {
                        Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                        None => Some(len),
                    };
                };

                // len_to_opの更新
                if let Some(len) = line.len_to_op() {
                    self.max_len_to_op = match self.max_len_to_op {
                        Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                        None => Some(len),
                    };
                };
                self.contents.push(Content::Line(line));
            }
            Content::SeparatedLines(ls) => {
                self.contents.push(Content::SeparatedLines(ls));
            }
        }
    }

    /// AS句で揃えたものを返す
    pub fn render(&mut self) -> String {
        let mut result = String::new();

        let mut is_first_line = true;

        // 再帰的に再構成した木を見る
        for content in self.contents.clone() {
            match content {
                Content::Line(line) => {
                    //ネスト分だけ\tを挿入
                    for current_depth in 0..self.depth {
                        // 1つ上のネストにsepを挿入
                        // ex)
                        //     depth = 2
                        //     sep = ","
                        //     の場合
                        //
                        //     "\t,\thoge"

                        if current_depth == self.depth - 1 {
                            if is_first_line {
                                is_first_line = false;
                            } else {
                                result.push_str(self.separetor.as_ref());
                            }
                        }
                        result.push_str("\t");
                    }

                    let mut current_len = 0;

                    for i in 0..line.contents().len() {
                        let element = line.elements.get(i).unwrap();

                        // as, opなどまでの最大長とその行での長さを引数にとる
                        // 現在見ているcontentがas, opであれば、必要な数\tを挿入する
                        let mut insert_tab =
                            |max_len_to: Option<usize>, len_to: Option<usize>| -> () {
                                if let (Some(max_len_to), Some(len_to)) = (max_len_to, len_to) {
                                    if current_len == len_to {
                                        let num_tab = (max_len_to / TAB_SIZE) - (len_to / TAB_SIZE);
                                        for _ in 0..num_tab {
                                            result.push_str("\t");
                                        }
                                    };
                                };
                            };

                        // ASの位置揃え
                        insert_tab(self.max_len_to_as, line.len_to_as());
                        // OPの位置揃え
                        insert_tab(self.max_len_to_op, line.len_to_op());

                        result.push_str(element);

                        //最後のelement以外は"\t"を挿入
                        if i != line.contents().len() - 1 {
                            result.push('\t');
                        }

                        // element.len()より大きく、かつTAB_SIZEの倍数のうち最小のものを足す
                        current_len += TAB_SIZE * (element.len() / TAB_SIZE + 1);
                    }

                    result.push_str("\n");
                }
                Content::SeparatedLines(mut sl) => {
                    // 再帰的にrender()を呼び、結果をresultに格納
                    let mut sl_res = sl.render();

                    if is_first_line {
                        is_first_line = false;
                    } else {
                        // sl.depth() - 1の位置にsepを挿入
                        //
                        // ex) sepがORの場合
                        //
                        //     t.n     =   5
                        // AND test    =   1
                        //
                        // が返ってきたら
                        //
                        // OR  t.n     =   5
                        // AND test    =   1

                        if sl.depth != 0 {
                            sl_res.insert_str(sl.depth - 1, &self.separetor);
                        }
                    }

                    result.push_str(&sl_res);
                }
            }
        }
        result
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
    pub fn format_sql(&mut self, node: Node, src: &str) -> SeparatedLines {
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

    fn format_source(&mut self, node: Node, src: &str) -> SeparatedLines {
        // source_file -> _statement*

        let mut result = SeparatedLines::new(self.state.depth, "");

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
            result.add_content(self.format_select_stmt(stmt_node, src));

            //select_statement以外も追加した場合この部分は削除
            // self.goto_not_comment_next_sibiling(buf, &mut cursor, src);
        }

        result
    }

    // SELECT文
    fn format_select_stmt(&mut self, node: Node, src: &str) -> Content {
        /*
            _select_statement ->
                select_clause
                from_clause?
                << 未対応!! join_clause* >>
                where_clause?
                << 未対応!! ... >>
        */

        let mut result = SeparatedLines::new(self.state.depth, "");

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let select_clause_node = cursor.node();

            result.add_content(self.format_select_clause(select_clause_node, src));
        }
        // else {
        //     return ();
        // }

        loop {
            // 次の兄弟へ移動
            // if !self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            //     break; // 子供がいなくなったら脱出
            // }
            if !cursor.goto_next_sibling() {
                break;
            }

            let clause_node = cursor.node();

            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // 最初は必ずFROM
                        let mut line = Line::new();
                        line.add_element("FROM");
                        result.add_content(Content::Line(line));

                        self.nest();
                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",");

                        // commaSep
                        // selectのときと同じであるため、統合したい
                        // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
                        if cursor.goto_next_sibling() {
                            let expr_node = cursor.node();

                            separated_lines.add_content(self.format_aliasable_expr(expr_node, src));
                        }

                        // while self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
                        while cursor.goto_next_sibling() {
                            let child_node = cursor.node();

                            match child_node.kind() {
                                "," => {
                                    continue;
                                }
                                _ => {
                                    separated_lines
                                        .add_content(self.format_aliasable_expr(child_node, src));
                                }
                            };
                        }

                        result.add_content(Content::SeparatedLines(separated_lines));
                        // buf.push_str(separated_lines.render().as_ref());

                        cursor.goto_parent();
                        self.unnest();
                    }
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    if cursor.goto_first_child() {
                        let mut line = Line::new();
                        line.add_element("WHERE");

                        self.nest();
                        let mut separated_lines = SeparatedLines::new(self.state.depth, "AND");

                        result.add_content(Content::Line(line));

                        // self.goto_not_comment_next_sibiling(buf, &mut cursor, src);
                        cursor.goto_next_sibling();

                        //expr
                        let expr_node = cursor.node();
                        let expr = self.format_expr(expr_node, src);
                        separated_lines.add_content(expr);
                        // buf.push_str(bool_expr.as_str());

                        eprintln!("{:#?}", separated_lines);
                        result.add_content(Content::SeparatedLines(separated_lines));
                        self.unnest();
                        cursor.goto_parent();
                    }
                }
                _ => {
                    break;
                }
            }
        }

        Content::SeparatedLines(result)
    }

    // SELECT句
    fn format_select_clause(&mut self, node: Node, src: &str) -> Content {
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
        let mut result = SeparatedLines::new(self.state.depth, "");

        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            // select_clauseの最初の子供は必ず"SELECT"であるはず

            let mut line = Line::new();
            line.add_element("SELECT");
            result.add_content(Content::Line(line));

            // if self.goto_not_comment_next_sibiling(buf, &mut cursor, src) {
            if cursor.goto_next_sibling() {
                // ここで、cursorはselect_clause_bodyを指している

                // 子供に移動
                cursor.goto_first_child();

                // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

                // 最初のノードは_aliasable_expressionのはず
                let expr_node = cursor.node();

                self.nest();
                let mut sepapated_lines = SeparatedLines::new(self.state.depth, ",");

                let content = self.format_aliasable_expr(expr_node, src);
                sepapated_lines.add_content(content);

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
                            let content = self.format_aliasable_expr(child_node, src);
                            sepapated_lines.add_content(content);
                        }
                    }
                }

                result.add_content(Content::SeparatedLines(sepapated_lines));
            }
        }

        Content::SeparatedLines(result)
    }

    // エイリアス可能な式
    fn format_aliasable_expr(&mut self, node: Node, src: &str) -> Content {
        /*
            _aliasable_expression ->
                alias | _expression

            alias ->
                _expression
                "AS"?
                identifier
                << 未対応!! "(" identifier ("," identifier)* ")" >>
        */

        // aliasable_expressionは1行と仮定(要修正)
        let mut line = Line::new();

        if node.kind() == "alias" {
            let mut cursor = node.walk();
            cursor.goto_first_child();

            // _expression
            let expr_line = self.format_expr(cursor.node(), src);
            match expr_line {
                Content::Line(ln) => {
                    line.append(ln);
                }
                Content::SeparatedLines(_) => {
                    //未対応
                }
            }

            // ("AS"? identifier)?
            if self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src) {
                // "AS"?

                if cursor.node().kind() == "AS" {
                    line.add_as("AS");
                    self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                }

                // identifier
                if cursor.node().kind() == "identifier" {
                    line.add_element(cursor.node().utf8_text(src.as_bytes()).unwrap());
                }
            }
        } else {
            let res = self.format_expr(node, src);
            match res {
                Content::Line(ln) => line = ln,
                Content::SeparatedLines(_) => {
                    //未対応
                }
            }
        }

        Content::Line(line)
    }

    // 式
    fn format_expr(&mut self, node: Node, src: &str) -> Content {
        let res: Content;

        match node.kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*

                let mut line = Line::new();
                let mut cursor = node.walk();
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
                line.add_element(result.as_str());

                res = Content::Line(line);
            }
            "binary_expression" => {
                let mut line = Line::new();

                let mut cursor = node.walk();
                cursor.goto_first_child();

                // 左辺
                let lhs_node = cursor.node();
                let lhs_line = self.format_expr(lhs_node, src);
                match lhs_line {
                    Content::Line(ln) => {
                        line.append(ln);
                    }
                    Content::SeparatedLines(_) => {
                        //右辺が複数行の場合は未対応
                    }
                }

                // 演算子
                // self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                cursor.goto_next_sibling();
                let op_node = cursor.node();
                line.add_op(op_node.utf8_text(src.as_bytes()).unwrap());

                // 右辺
                self.goto_not_comment_next_sibiling_for_line(&mut line, &mut cursor, src);
                let rhs_node = cursor.node();
                let expr_line = self.format_expr(rhs_node, src);

                match expr_line {
                    Content::Line(ln) => {
                        line.append(ln);
                    }
                    Content::SeparatedLines(_) => {
                        //右辺が複数行の場合は未対応
                    }
                }
                res = Content::Line(line);
            }
            "boolean_expression" => {
                res = self.format_bool_expr(node, src);
            }
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                let mut line = Line::new();
                line.add_element(node.utf8_text(src.as_bytes()).unwrap());
                res = Content::Line(line);
            }
            _ => {
                let mut line = Line::new();
                eprintln!("format_expr(): unknown node ({}).", node.kind());
                line.add_element(self.format_straightforward(node, src).as_ref());
                res = Content::Line(line);
            }
        }

        res
    }

    // boolean_expression: $ =>
    // choice(
    //   prec.left(PREC.unary, seq(kw("NOT"), $._expression)),
    //   prec.left(PREC.and, seq($._expression, kw("AND"), $._expression)),
    //   prec.left(PREC.or, seq($._expression, kw("OR"), $._expression)),
    // ),
    fn format_bool_expr(&mut self, node: Node, src: &str) -> Content {
        let mut sep_lines = SeparatedLines::new(self.state.depth, "");

        let mut cursor = node.walk();

        cursor.goto_first_child();

        if cursor.node().kind() == "NOT" {
            // 未対応
        } else {
            sep_lines.add_content(self.format_expr(cursor.node(), src));

            cursor.goto_next_sibling();

            sep_lines.set_separetor(cursor.node().kind());

            cursor.goto_next_sibling();

            sep_lines.add_content(self.format_expr(cursor.node(), src));
        }
        Content::SeparatedLines(sep_lines)
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
