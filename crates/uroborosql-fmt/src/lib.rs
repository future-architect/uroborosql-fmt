pub mod config;
mod cst;
pub mod error;
mod re;
mod two_way_sql;
mod util;
mod validate;
mod visitor;

use crate::validate::validate_format_result;
use config::*;
use error::UroboroSQLFmtError;
use postgresql_cst_parser::tree_sitter::parse;
use postgresql_cst_parser::tree_sitter::parse_2way;
use postgresql_cst_parser::tree_sitter::Node;
use two_way_sql::format_two_way_sql;
use two_way_sql::is_two_way_sql;
use visitor::Visitor;

use std::fmt::Write;

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
            "use_parser_error_recovery: {}",
            config.use_parser_error_recovery
        );
    }

    // パーサの2way-sql用エラー回復機能を使うかどうか
    let use_parser_error_recovery = config.use_parser_error_recovery;

    let parse_result = if use_parser_error_recovery {
        parse_2way(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))
    } else {
        parse(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))
    };

    match parse_result {
        // パースできるSQLはそのままフォーマットする
        Ok(tree) => {
            if CONFIG.read().unwrap().debug {
                eprintln!("mode: normal");
            }

            validate_format_result(src, false)?;
            load_settings(config);

            format_cst(&tree, src)
        }
        // パース出来ないSQLは、それが 2way-sqlならば2way-sqlモードでフォーマットする
        // 2way-sqlでもない場合はパースエラーとして返す
        Err(e) => {
            if is_two_way_sql(src) {
                if CONFIG.read().unwrap().debug {
                    eprintln!("mode: 2way-sql");
                }

                validate_format_result(src, true)?;
                load_settings(config);

                format_two_way_sql(src)
            } else {
                Err(e)
            }
        }
    }
}

pub(crate) fn format(src: &str) -> Result<String, UroboroSQLFmtError> {
    let tree = if CONFIG.read().unwrap().use_parser_error_recovery {
        parse_2way(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))?
    } else {
        parse(src).map_err(|e| UroboroSQLFmtError::ParseError(format!("{e:?}")))?
    };

    format_cst(&tree, src)
}

/// 渡されたTreeをもとにフォーマットする
pub(crate) fn format_cst(
    tree: &postgresql_cst_parser::tree_sitter::Tree,
    src: &str,
) -> Result<String, UroboroSQLFmtError> {
    if CONFIG.read().unwrap().debug {
        eprintln!("CST:");
        print_cst(tree.root_node(), 0);
    }

    // ビジターオブジェクトを生成
    let mut visitor = Visitor::default();

    // SQLソースファイルをフォーマット用構造体に変換する
    let stmts = visitor.visit_sql(tree.root_node(), src.as_ref())?;

    if CONFIG.read().unwrap().debug {
        eprintln!();
        eprintln!("{stmts:#?}");
    }

    let result = stmts
        .iter()
        .map(|stmt| stmt.render(0).expect("render: error"))
        .collect();

    Ok(result)
}

fn print_cst(node: Node, depth: usize) {
    let mut output = String::new();
    write_cst(node, depth, &mut output).expect("debug: writing to string should not fail");
    eprintln!("{output}");
}

fn write_cst(node: Node, depth: usize, output: &mut String) -> std::fmt::Result {
    for _ in 0..depth {
        write!(output, "  ")?;
    }
    write!(
        output,
        "{} [{}-{}]",
        node.kind(),
        node.start_position(),
        node.end_position()
    )?;

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            writeln!(output)?;
            write_cst(cursor.node(), depth + 1, output)?;
            //次の兄弟ノードへカーソルを移動
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }

    Ok(())
}
