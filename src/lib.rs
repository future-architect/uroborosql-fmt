use tree_sitter::Node;

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
    contents: Vec<String>,      // lifetimeの管理が面倒なのでStringに
    len: usize,
    len_to_op: Option<usize>,   // = までの距離
}

impl Line {
    pub fn new() -> Line {
        Line {
            contents: vec![],
            len: 0,
            len_to_op: None,
        }
    }

    /// 行の要素を足す(演算子はadd_operator()を使う)
    pub fn add_content(&mut self, content: &str) {
        self.len += content.len();
        self.contents.push(content.to_ascii_uppercase());
    }

    /// 演算子を追加する
    pub fn add_operator(&mut self, op: &str) {
        self.len_to_op = Some(self.len);
        self.add_content(op);
    }

    pub fn len_to_op(&self) -> Option<usize> {
        self.len_to_op
    }
}

struct SeparatedLines {
    separetor: String,
    lines: Vec<Line>,
    max_len_to_op: Option<usize>,
}

impl SeparatedLines {
    pub fn new(sep: &str) -> SeparatedLines {
        SeparatedLines {
            separetor: sep.to_string(),
            lines: vec![],
            max_len_to_op: None
        }
    }

    pub fn add_line(&mut self, line: Line) {
        if let Some(len) = line.len_to_op() {
            self.max_len_to_op = match self.max_len_to_op {
                Some(maxlen) => Some(std::cmp::max(len, maxlen)),
                None => Some(len)
            };
        };

        self.lines.push(line);
    }
}

pub struct Formatter {
    state: FormatterState,
}

impl Formatter {
    pub fn new() -> Formatter {
        Formatter {
            state: FormatterState{
                depth: 0
            }
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
                break;  // 子供がいなくなったら脱出
            }

            let clause_node = cursor.node();

            eprintln!("{}", clause_node.kind());
            match clause_node.kind() {
                "from_clause" => {
                    if cursor.goto_first_child() {
                        // 最初は必ずFROM
                        self.push_indent(buf);
                        buf.push_str("FROM\n");
                        
                        // commaSep
                        // selectのときと同じであるため、統合したい
                        if cursor.goto_next_sibling() {
                            let expr_node = cursor.node();
                            
                            self.state.depth += 1;
                            self.push_indent(buf);
                            self.format_aliasable_expr(buf, expr_node, src);
                            buf.push_str("\n");

                            self.state.depth -= 1;
                        }
                        
                        while cursor.goto_next_sibling() {
                            let child_node = cursor.node();
                            match child_node.kind() {
                                "," => {
                                    self.push_indent(buf);
                                    buf.push_str(",");
                                }
                                _ => {
                                    
                                    buf.push_str("\t");
                                    self.format_aliasable_expr(buf, child_node, src);
                                    buf.push_str("\n");
                                }
                            }
                        }

                        cursor.goto_parent();
                    }
                },
                // where_clause: $ => seq(kw("WHERE"), $._expression),
                "where_clause" => {                    
                    if cursor.goto_first_child() {

                        self.push_indent(buf);
                        buf.push_str("WHERE\n");
                        
                        cursor.goto_next_sibling();
                        self.state.depth += 1;
                        self.push_indent(buf);
                        self.format_expr(buf, cursor.node(), src);
                        
                        buf.push_str("\n");
                        
                        self.state.depth -= 1;
                        
                        cursor.goto_parent();
                    }
                },
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
                
                self.state.depth += 1;

                self.push_indent(buf);
                self.format_aliasable_expr(buf, expr_node, src);
                buf.push_str("\n");

                self.state.depth -= 1;
            }
            
            // (',' _aliasable_expression)*
            while cursor.goto_next_sibling() {
                let child_node = cursor.node();
                match child_node.kind() {
                    "," => {
                        self.push_indent(buf);
                        buf.push_str(",");
                    }
                    _ => {
                        buf.push_str("\t");
                        self.format_aliasable_expr(buf, child_node, src);
                        buf.push_str("\n");
                    }
                }
            }
       }
    }

    // エイリアス可能な式
    fn format_aliasable_expr(&mut self, buf: &mut String, node: Node, src: &str) {
        /*
            _aliasable_expression ->
                alias | _expression
                
            alias ->
                _expression
                "AS"?
                identifier
                << 未対応!! "(" identifier ("," identifier)* ")" >>
        */
        let mut cursor = node.walk();
        
        // _expression
        self.format_expr(buf, node, src);
        
        // ("AS"? identifier)?
        if cursor.goto_next_sibling() {
            // "AS"?
            if cursor.node().kind() == "AS" {
                buf.push_str("\tAS\t");
                cursor.goto_next_sibling();
            }
            
            // identifier
            if cursor.node().kind() == "identifier" {
                buf.push_str(node.utf8_text(src.as_bytes()).unwrap());
            }
        }
    }

    // 式
    fn format_expr(&mut self, buf: &mut String, node: Node, src: &str) {
        match node.kind() {
            "dotted_name" => {
                // dotted_name -> identifier ("." identifier)*
                let mut cursor = node.walk();
                cursor.goto_first_child();
                
                let id_node = cursor.node();
                buf.push_str(id_node.utf8_text(src.as_bytes()).unwrap());
                
                while cursor.goto_next_sibling() {
                    match cursor.node().kind() {
                        "." => buf.push_str("."),
                        _ => buf.push_str(cursor.node().utf8_text(src.as_bytes()).unwrap()),
                    };
                }
            },
            "binary_expression" => {
                let mut cursor = node.walk();
                cursor.goto_first_child();
                
                // 左辺
                let lhs_node = cursor.node();
                self.format_expr(buf, lhs_node, src);
                
                // 演算子
                cursor.goto_next_sibling();
                let op_node = cursor.node();
                buf.push_str("\t");
                buf.push_str(op_node.utf8_text(src.as_bytes()).unwrap());
                buf.push_str("\t");
                
                // 右辺
                cursor.goto_next_sibling();
                let rhs_node = cursor.node();
                self.format_expr(buf, rhs_node, src);
            },
            // identifier | number | string (そのまま表示)
            "identifier" | "number" | "string" => {
                buf.push_str(node.utf8_text(src.as_bytes()).unwrap());
            }
            _ => self.format_straightforward(buf, node, src),
        }
        
    }

    // 未対応の構文をそのまま表示する
    fn format_straightforward(&mut self, buf: &mut String, node: Node, src: &str) {
        if node.child_count() <= 0 {
            buf.push_str(node.utf8_text(src.as_bytes()).unwrap());
            buf.push_str("\n");
        }

        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            self.format_straightforward(buf, cursor.node(), src);
            while cursor.goto_next_sibling() {
                self.format_straightforward(buf, cursor.node(), src)
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

    dfs(root_node, 0);
    eprintln!("");
}

fn dfs(node: Node, depth: usize) {
    for _ in 0..depth {
        eprint!("  ");
    }
    eprint!("({} [{}-{}]", node.kind(), node.start_position(), node.end_position());

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        eprintln!("");
        dfs(cursor.node(), depth + 1);
        while cursor.goto_next_sibling() {
            eprintln!("");
            dfs(cursor.node(), depth + 1);
        }
        eprint!(")");
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
}
