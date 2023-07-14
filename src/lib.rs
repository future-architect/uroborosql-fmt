mod config;
mod cst;
mod formatter;
mod re;
mod two_way_sql;
mod util;
mod validate;

use config::*;
pub use cst::UroboroSQLFmtError;
use formatter::Formatter;

use tree_sitter::{Language, Node};
use two_way_sql::{format_two_way_sql, is_two_way_sql};
use validate::validate_format_result;

/// 引数のSQLをフォーマットして返す
pub fn format_sql(src: &str, config_path: Option<&str>) -> Result<String, UroboroSQLFmtError> {
    // tree-sitter-sqlの言語を取得
    let language = tree_sitter_sql::language();

    let is_two_way_sql = is_two_way_sql(src);

    validate_format_result(src, language, is_two_way_sql)?;

    //設定ファイルの読み込み
    if let Some(path) = config_path {
        load_settings(path)?
    } else {
        // 指定されていない場合は、デフォルトの設定をロードする。
        // テスト等で複数回format_sql()を呼び出す場合に必要になる。
        // ここでロードしないと、マージ検証処理で設定を変更しているため、
        // 2回目以降の呼び出しで、マージ検証用の設定が使われてしまう。
        load_default_settings()
    }

    if is_two_way_sql {
        // 2way-sqlモード
        if CONFIG.read().unwrap().debug {
            eprintln!("\n{} 2way-sql mode {}\n", "=".repeat(20), "=".repeat(20));
        }

        format_two_way_sql(src, language)
    } else {
        // ノーマルモード
        if CONFIG.read().unwrap().debug {
            eprintln!("\n{} normal mode {}\n", "=".repeat(20), "=".repeat(20));
        }

        format(src, language)
    }
}

pub(crate) fn format(src: &str, language: Language) -> Result<String, UroboroSQLFmtError> {
    // パーサオブジェクトを生成
    let mut parser = tree_sitter::Parser::new();
    // tree-sitter-sqlの言語をパーサにセットする
    parser.set_language(language).unwrap();
    // srcをパースし、結果のTreeを取得
    let tree = parser.parse(src, None).unwrap();
    // Treeのルートノードを取得
    let root_node = tree.root_node();

    if CONFIG.read().unwrap().debug {
        print_cst(root_node, 0);
        eprintln!();
    }

    // フォーマッタオブジェクトを生成
    let mut formatter = Formatter::default();

    // formatを行い、バッファに結果を格納
    let stmts = formatter.format_sql(root_node, src.as_ref())?;

    if CONFIG.read().unwrap().debug {
        eprintln!("{:#?}", stmts);
    }

    let result = stmts
        .iter()
        .map(|stmt| stmt.render(0).expect("render: error"))
        .collect();

    Ok(result)
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
