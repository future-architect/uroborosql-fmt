mod cst;
mod formatter;

use formatter::Formatter;
use tree_sitter::Node;

pub(crate) const DEBUG_MODE: bool = true; // デバッグモード

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

    if DEBUG_MODE {
        dfs(root_node, 0);
        eprintln!();
    }

    // フォーマッタオブジェクトを生成
    let mut formatter = Formatter::default();

    // formatを行い、バッファに結果を格納
    let stmts = formatter.format_sql(root_node, src.as_ref());

    if DEBUG_MODE {
        eprintln!("{:#?}", stmts);
    }

    let result = stmts
        .iter()
        .map(|stmt| stmt.render().expect("render: error"))
        .collect();

    result
}

// cstを表示する関数(デバッグ用)
// fn print_cst(src: &str) {
//     // tree-sitter-sqlの言語を取得
//     let language = tree_sitter_sql::language();
//     // パーサオブジェクトを生成
//     let mut parser = tree_sitter::Parser::new();
//     // tree-sitter-sqlの言語をパーサにセットする
//     parser.set_language(language).unwrap();

//     // srcをパースし、結果のTreeを取得
//     let tree = parser.parse(&src, None).unwrap();
//     // Treeのルートノードを取得
//     let root_node = tree.root_node();
// }

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
