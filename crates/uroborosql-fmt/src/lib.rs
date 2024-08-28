pub mod config;
mod cst;
pub mod error;
mod re;
mod two_way_sql;
mod util;
mod validate;
mod visitor;

use config::*;
use error::UroboroSQLFmtError;
use visitor::Visitor;

use tree_sitter::{Language, Node, Tree};
use two_way_sql::{format_two_way_sql, is_two_way_sql};
use validate::validate_format_result;

/// 設定ファイルより優先させるオプションを JSON 文字列で与えて、SQLのフォーマットを行う。
///
/// Format sql with json string that describes higher priority options than the configuration file.
pub fn format_sql(
    src: &str,
    settings_json: Option<&str>,
    config_path: Option<&str>,
) -> Result<String, UroboroSQLFmtError> {
    let config = Config::new(settings_json, config_path)?;

    format_sql_with_config(src, config)
}

/// 設定をConfig構造体で渡して、SQLをフォーマットする。
pub(crate) fn format_sql_with_config(
    src: &str,
    config: Config,
) -> Result<String, UroboroSQLFmtError> {
    // tree-sitter-sqlの言語を取得
    let language = tree_sitter_sql::language();

    let is_two_way_sql = is_two_way_sql(src);

    validate_format_result(src, language, is_two_way_sql)?;

    load_settings(config);

    // パーサオブジェクトを生成
    let mut parser = tree_sitter::Parser::new();
    // tree-sitter-sqlの言語をパーサにセットする
    parser.set_language(language).unwrap();
    // srcをパースし、結果のTreeを取得
    let tree = parser.parse(src, None).unwrap();
    let has_syntax_error = has_syntax_error(&tree);

    if is_two_way_sql && has_syntax_error {
        // 2way-sqlモードでフォーマットする
        if CONFIG.read().unwrap().debug {
            eprintln!("\n{} 2way-sql mode {}\n", "=".repeat(20), "=".repeat(20));
        }

        format_two_way_sql(src, language)
    } else {
        // ノーマルモード
        if CONFIG.read().unwrap().debug {
            eprintln!("\n{} normal mode {}\n", "=".repeat(20), "=".repeat(20));
        }

        format_tree(tree, src)
    }
}

pub(crate) fn format(src: &str, language: Language) -> Result<String, UroboroSQLFmtError> {
    // パーサオブジェクトを生成
    let mut parser = tree_sitter::Parser::new();
    // tree-sitter-sqlの言語をパーサにセットする
    parser.set_language(language).unwrap();
    // srcをパースし、結果のTreeを取得
    let tree = parser.parse(src, None).unwrap();
    format_tree(tree, src)
}

/// 渡されたTreeをもとにフォーマットする
pub(crate) fn format_tree(tree: Tree, src: &str) -> Result<String, UroboroSQLFmtError> {
    // Treeのルートノードを取得
    let root_node = tree.root_node();

    if CONFIG.read().unwrap().debug {
        print_cst(root_node, 0);
        eprintln!();
    }

    // ビジターオブジェクトを生成
    let mut visitor = Visitor::default();

    // SQLソースファイルをフォーマット用構造体に変換する
    let stmts = visitor.visit_sql(root_node, src.as_ref())?;

    if CONFIG.read().unwrap().debug {
        eprintln!("{stmts:#?}");
    }

    let result = stmts
        .iter()
        .map(|stmt| stmt.render(0).expect("render: error"))
        .collect();

    Ok(result)
}

fn has_syntax_error(tree: &Tree) -> bool {
    tree.root_node().has_error()
}

/// CSTを出力 (デバッグ用)
fn print_cst(node: Node, depth: usize) {
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
            print_cst(cursor.node(), depth + 1);
            //次の兄弟ノードへカーソルを移動
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
