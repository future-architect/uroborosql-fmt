pub mod config;
mod cst;
pub mod error;
mod re;
mod two_way_sql;
mod util;
mod validate;
mod visitor;

mod new_visitor;
mod pg_validate;

use config::*;
use error::UroboroSQLFmtError;
use new_visitor::Visitor as NewVisitor;
use postgresql_cst_parser::tree_sitter::parse as pg_parse;
use postgresql_cst_parser::tree_sitter::parse_2way as pg_parse_2way;
use two_way_sql::pg_format_two_way_sql;
use visitor::Visitor;

use tree_sitter::{Language, Node, Tree};
use two_way_sql::{format_two_way_sql, is_two_way_sql};
use validate::validate_format_result;

use crate::pg_validate::validate_format_result as pg_validate_format_result;

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
    if CONFIG.read().unwrap().debug {
        eprintln!(
            "use_parser_error_recovery = {}",
            config.use_parser_error_recovery
        );
        eprintln!("parser: postgresql-cst-parser");
    }

    // パーサの2way-sql用エラー回復機能を使うかどうか
    let use_parser_error_recovery = config.use_parser_error_recovery;

    let parse_result = if use_parser_error_recovery {
        pg_parse_2way(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))
    } else {
        pg_parse(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))
    };

    match parse_result {
        // パースできるSQLはそのままフォーマットする
        Ok(tree) => {
            if CONFIG.read().unwrap().debug {
                eprintln!("mode: normal");
            }

            pg_validate_format_result(src, false)?;
            load_settings(config);

            pg_format_cst(&tree, src)
        }
        // パース出来ないSQLは、それが 2way-sqlならば2way-sqlモードでフォーマットする
        // 2way-sqlでもない場合はパースエラーとして返す
        Err(e) => {
            if is_two_way_sql(src) {
                if CONFIG.read().unwrap().debug {
                    eprintln!("mode: 2way-sql");
                }

                pg_validate_format_result(src, true)?;
                load_settings(config);

                pg_format_two_way_sql(src)
            } else {
                Err(e)
            }
        }
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

#[allow(unused)]
fn pg_print_cst(node: postgresql_cst_parser::tree_sitter::Node, depth: usize) {
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
            pg_print_cst(cursor.node(), depth + 1);
            //次の兄弟ノードへカーソルを移動
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

pub(crate) fn pg_format(src: &str) -> Result<String, UroboroSQLFmtError> {
    let tree = if CONFIG.read().unwrap().use_parser_error_recovery {
        pg_parse_2way(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))?
    } else {
        pg_parse(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))?
    };

    pg_format_cst(&tree, src)
}

/// 渡されたTreeをもとにフォーマットする
pub(crate) fn pg_format_cst(
    tree: &postgresql_cst_parser::tree_sitter::Tree,
    src: &str,
) -> Result<String, UroboroSQLFmtError> {
    // if CONFIG.read().unwrap().debug {
    //     eprintln!("CST: {:#?}", tree);
    // }

    // ビジターオブジェクトを生成
    let mut visitor = NewVisitor::default();

    // SQLソースファイルをフォーマット用構造体に変換する
    let stmts = visitor.visit_sql(tree.root_node(), src.as_ref())?;

    // if CONFIG.read().unwrap().debug {
    //     eprintln!("{stmts:#?}");
    // }

    let result = stmts
        .iter()
        .map(|stmt| stmt.render(0).expect("render: error"))
        .collect();

    Ok(result)
}
