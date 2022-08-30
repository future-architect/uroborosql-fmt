use tree_sitter::Node;

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
    // 結果を格納するバッファ
    let mut result = String::new();
    // formatを行い、バッファに結果を格納
    formatter.format_sql(&mut result, root_node, src.as_ref());

    result
}

/// インデントの深さや位置をそろえるための情報を保持する構造体
struct FormatterState {
    pub depth: usize,
}

struct Line {
    contents: Vec<String>, // lifetimeの管理が面倒なのでStringに
    len: usize,
    len_to_as: Option<usize>, // AS までの距離
    len_to_op: Option<usize>, // 演算子までの距離(1行に一つ)
}

impl Line {
    pub fn new() -> Line {
        Line {
            contents: vec![] as Vec<String>,
            len: 0,
            len_to_as: None,
            len_to_op: None,
        }
    }

    pub fn contents(&self) -> &Vec<String> {
        &self.contents
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
    pub fn add_content(&mut self, content: &str) {
        self.len += content.len();
        self.contents.push(content.to_ascii_uppercase());
    }

    /// AS句を追加する
    pub fn add_as(&mut self, as_str: &str) {
        self.len_to_as = Some(self.len);
        self.add_content(as_str);
    }

    /// 演算子を追加する
    pub fn add_op(&mut self, op_str: &str) {
        self.len_to_op = Some(self.len);
        self.add_content(op_str);
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
            self.contents.push(content.to_string());
        }
    }

    /// contentsを"\t"でjoinして返す
    pub fn to_string(&self) -> String {
        self.contents.join("\t")
    }
}

struct SeparatedLines {
    depth: usize,                 // インデントの深さ
    separetor: String,            // セパレータ(e.g., ',', AND)
    lines: Vec<Line>,             // 各行の情報
    max_len_to_as: Option<usize>, // ASまでの最長の長さ
    max_len_to_op: Option<usize>, // 演算子までの最長の長さ(1行に一つと仮定)
}

impl SeparatedLines {
    pub fn new(depth: usize, sep: &str) -> SeparatedLines {
        SeparatedLines {
            depth,
            separetor: sep.to_string(),
            lines: vec![] as Vec<Line>,
            max_len_to_as: None,
            max_len_to_op: None,
        }
    }

    /// Line構造体を追加
    pub fn add_line(&mut self, line: Line) {
        if let Some(len) = line.len_to_as() {
            self.max_len_to_as = match self.max_len_to_as {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

        if let Some(len) = line.len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len),
            };
        };

        self.lines.push(line);
    }

    /// AS句で揃えたものを返す
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        let mut is_first = true;
        for line in (&self.lines).into_iter() {
            //ネスト分だけ\tを挿入
            for _ in 0..self.depth {
                result.push_str("\t");
            }

            if is_first {
                // 最初の行は\tから始まる
                is_first = false;
                result.push_str("\t");
            } else {
                // 2行目以降は sep \t から始まる
                result.push_str(self.separetor.as_ref());
                result.push_str("\t");
            }

            if let (Some(max_len_to_as), Some(len_to_as))
                = (self.max_len_to_as, line.len_to_as())
            {   // AS
                let mut current_len = 0;    // 読み込んだcontentの長さの合計
                for content in line.contents().into_iter() {
                    if current_len == len_to_as {   // "AS"に到達したとき
                        let num_len
                            = (max_len_to_as / TAB_SIZE) - (len_to_as / TAB_SIZE);
                        // num_tabだけ\tを挿入
                        for _ in 0..num_len {
                            result.push_str("\t");
                        }
                        // ASを挿入
                        result.push_str("\t");
                        result.push_str(content.as_str());
                        result.push_str("\t");
                    } else {
                        result.push_str(content);
                    }
                    current_len += content.len();
                }
            // 演算子がある行
            // ASとほとんど同じ処理なので、統合予定
            } else if let (Some(max_len_to_op), Some(len_to_op))
                = (self.max_len_to_op, line.len_to_op())
            {
                let mut current_len = 0;    // 読み込んだcontentの長さの合計
                for content in line.contents().into_iter() {
                    if current_len == len_to_op {   // 演算子に到達したとき
                        let num_len
                            = (max_len_to_op / TAB_SIZE) - (len_to_op / TAB_SIZE);
                        // num_tabだけ\tを挿入
                        for _ in 0..num_len {
                            result.push_str("\t");
                        }
                        // ASを挿入
                        result.push_str("\t");
                        result.push_str(content.as_str());
                        result.push_str("\t");
                    } else {
                        result.push_str(content);
                    }
                    current_len += content.len();
                }
            } else {
                result.push_str(line.to_string().as_ref());
            }
            result.push_str("\n");
        }

        result
    }
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
    pub fn format_sql(&mut self, buf: &mut String, node: Node, src: &str) {
        self.format_source(buf, node, src);
    }

    // インデントをbufにプッシュする
    fn push_indent(&mut self, buf: &mut String) {
        for _ in 0..self.state.depth {
            buf.push_str("\t");
        }
    }

    // ネストを1つ深くする
    fn nest(&mut self) {
        self.state.depth += 1;
    }

    //ネストを1つ浅くする
    fn unnest(&mut self) {
        self.state.depth -= 1;
    }

    fn format_source(&mut self, buf: &mut String, node: Node, src: &str) {
        // source_file -> _statement*

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let stmt_node = cursor.node();

            // 現状はselect_statementのみ
            self.format_select_stmt(buf, stmt_node, src);
        }
    }

    // SELECT文
    fn format_select_stmt(&mut self, buf: &mut String, node: Node, src: &str) {
        /*
            _select_statement ->
                select_clause
                from_clause?
                << 未対応!! join_clause* >>
                where_clause?
                << 未対応!! ... >>
        */

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            let select_clause_node = cursor.node();
            self.format_select_clause(buf, select_clause_node, src);
        } else {
            return ();
        }

        loop {
            // 次の兄弟へ移動
            if !cursor.goto_next_sibling() {
                break; // 子供がいなくなったら脱出
            }

            let clause_node = cursor.node();

            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // 最初は必ずFROM
                        self.push_indent(buf);
                        buf.push_str("FROM\n");

                        let mut separated_lines = SeparatedLines::new(self.state.depth, ",");

                        // commaSep
                        // selectのときと同じであるため、統合したい
                        if cursor.goto_next_sibling() {
                            let expr_node = cursor.node();

                            self.nest();
                            separated_lines.add_line(self.format_aliasable_expr(expr_node, src));
                            self.unnest();
                        }

                        while cursor.goto_next_sibling() {
                            let child_node = cursor.node();

                            match child_node.kind() {
                                "," => {
                                    continue;
                                }
                                _ => {
                                    separated_lines
                                        .add_line(self.format_aliasable_expr(child_node, src));
                                }
                            };
                        }

                        buf.push_str(separated_lines.to_string().as_ref());

                        cursor.goto_parent();
                    }
                }
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {
                    if cursor.goto_first_child() {
                        self.push_indent(buf);
                        buf.push_str("WHERE\n");

                        cursor.goto_next_sibling();
                        
                        let bool_expr_node = cursor.node();
                        let bool_expr_str = self.format_bool_expr(bool_expr_node, src, self.state.depth);
                        buf.push_str(bool_expr_str.as_str());

                        cursor.goto_parent();
                    }
                }
                _ => {
                    break;
                }
            }
        }
    }

    // SELECT句
    fn format_select_clause(&mut self, buf: &mut String, node: Node, src: &str) {
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

        let mut cursor = node.walk();

        if cursor.goto_first_child() {
            // select_clauseの最初の子供は必ず"SELECT"であるはず
            self.push_indent(buf);
            buf.push_str("SELECT\n");

            if cursor.goto_next_sibling() {
                // ここで、cursorはselect_clause_bodyを指している

                // 子供に移動
                cursor.goto_first_child();

                // select_clause_body -> _aliasable_expression ("," _aliasable_expression)*

                // 最初のノードは_aliasable_expressionのはず
                let expr_node = cursor.node();

                let mut sepapated_lines = SeparatedLines::new(self.state.depth, ",");

                self.nest();

                let line = self.format_aliasable_expr(expr_node, src);
                sepapated_lines.add_line(line);

                self.unnest();

                // (',' _aliasable_expression)*
                while cursor.goto_next_sibling() {
                    let child_node = cursor.node();
                    match child_node.kind() {
                        "," => {
                            continue;
                        }
                        _ => {
                            let line = self.format_aliasable_expr(child_node, src);
                            sepapated_lines.add_line(line);
                        }
                    }
                }

                let string = sepapated_lines.to_string();
                buf.push_str(string.as_str());
            }
        }
    }

    // エイリアス可能な式
    fn format_aliasable_expr(&mut self, node: Node, src: &str) -> Line {
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
            line.append(expr_line);

            // ("AS"? identifier)?
            if cursor.goto_next_sibling() {
                // "AS"?
                if cursor.node().kind() == "AS" {
                    line.add_as("AS");
                    cursor.goto_next_sibling();
                }

                // identifier
                if cursor.node().kind() == "identifier" {
                    line.add_content(cursor.node().utf8_text(src.as_bytes()).unwrap());
                }
            }
        } else {
            line = self.format_expr(node, src);
        }

        line
    }

    // 式
    fn format_expr(&mut self, node: Node, src: &str) -> Line {
        // expressionは1行と仮定する(boolean_exprssionなどは2行以上になったりするので要修正)
        let mut line = Line::new();

        match node.kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*
                let mut cursor = node.walk();
                cursor.goto_first_child();

                let mut result = String::new();

                let id_node = cursor.node();
                result.push_str(id_node.utf8_text(src.as_bytes()).unwrap());

                while cursor.goto_next_sibling() {
                    match cursor.node().kind() {
                        "." => result.push_str("."),
                        _ => result.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }
                line.add_content(result.as_str());
            }
            "binary_expression" => {
                let mut cursor = node.walk();
                cursor.goto_first_child();

                // 左辺
                let lhs_node = cursor.node();
                let lhs_line = self.format_expr(lhs_node, src);
                line.append(lhs_line);

                // 演算子
                cursor.goto_next_sibling();
                let op_node = cursor.node();
                line.add_op(op_node.utf8_text(src.as_bytes()).unwrap());

                // 右辺
                cursor.goto_next_sibling();
                let rhs_node = cursor.node();
                let expr_line = self.format_expr(rhs_node, src);
                line.append(expr_line);
            }
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                line.add_content(node.utf8_text(src.as_bytes()).unwrap());
            }
            _ => {
                eprintln!("format_expr(): unknown node ({}).", node.kind());
                line.add_content(self.format_straightforward(node, src).as_ref())
            }
        }

        line
    }

    fn format_bool_expr(&mut self, node: Node, src: &str, depth: usize) -> String {
        // 今はANDしか認めない
        let mut sep_lines = SeparatedLines::new(depth, "AND");
        
        // ブール式ではない
        if node.kind() != "boolean_expression" {
            let line = self.format_expr(node, src);
            sep_lines.add_line(line);
            return sep_lines.to_string();
        }
        
        let mut cursor = node.walk();

        let mut boolean_nest = 0;

        // boolean_expressionの最左に移動(NOT, BETWEEN対応のことは考えていない)
        while cursor.node().kind() == "boolean_expression" {
            boolean_nest += 1;
            cursor.goto_first_child();
        }

        // 一番左下の子
        let left_expr_node = cursor.node();
        let line = self.format_expr(left_expr_node, src);
        sep_lines.add_line(line);

        for _ in 0..boolean_nest {
            // 現状ではANDのみを認めているため演算子を読み飛ばす
            cursor.goto_next_sibling();
            
            // 右の子
            cursor.goto_next_sibling();
            let right_expr_node = cursor.node();
            let line = self.format_expr(right_expr_node, src);
            sep_lines.add_line(line);

            cursor.goto_parent();
        }
        
        sep_lines.to_string()
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_select() {
        let src = "SELECT A, B, C";
        let expect = "SELECT\n\tA\n,\tB\n,\tC\n";
        let actual = format_sql(src);
        assert_eq!(actual, expect);
    }

    #[test]
    fn test_from() {
        let src = "SELECT HOGE, FUGA FROM TABLE1";
        let expect = "SELECT\n\tHOGE\n,\tFUGA\nFROM\n\tTABLE1\n";
        let actual = format_sql(src);
        assert_eq!(actual, expect);
    }

    #[test]
    fn test_where() {
        let src = "SELECT A FROM T WHERE T.N = '1'";
        let expect = "SELECT\n\tA\nFROM\n\tT\nWHERE\n\tT.N\t=\t'1'\n";
        let actual = format_sql(src);
        assert_eq!(actual, expect);
    }

    #[test]
    fn test_align_as() {
        let src = "SELECT A FROM TAB1 AS T1, TABTABTABTAB AS L";
        let expect = "SELECT\n\tA\nFROM\n\tTAB1\t\t\tAS\tT1\n,\tTABTABTABTAB\tAS\tL\n";
        let actual = format_sql(src);
        assert_eq!(actual, expect);
    }
}
