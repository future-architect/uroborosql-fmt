use tree_sitter::{Language, Node, Tree};

use crate::{
    config::{load_never_complement_settings, CONFIG},
    format,
    formatter::COMMENT,
    print_cst,
    two_way_sql::format_two_way_sql,
    UroboroSQLFmtError,
};

/// フォーマット前後でSQLに欠落が生じないかを検証する。
pub(crate) fn validate_format_result(
    src: &str,
    language: Language,
    is_two_way_sql: bool,
) -> Result<(), UroboroSQLFmtError> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(language).unwrap();

    let src_ts_tree = parser.parse(src, None).unwrap();

    let dbg = CONFIG.read().unwrap().debug;

    // 補完を行わない設定に切り替える
    load_never_complement_settings();

    let format_result = if is_two_way_sql {
        format_two_way_sql(src, language)?
    } else {
        format(src, language)?
    };
    let dst_ts_tree = parser.parse(&format_result, None).unwrap();

    let validate_result = compare_tree(src, &format_result, &src_ts_tree, &dst_ts_tree);

    if dbg && validate_result.is_err() {
        eprintln!(
            "\n{} validation error! {}\n",
            "=".repeat(20),
            "=".repeat(20)
        );
        eprintln!("src_ts_tree =");
        print_cst(src_ts_tree.root_node(), 0);
        eprintln!();
        eprintln!("dst_ts_tree =");
        print_cst(dst_ts_tree.root_node(), 0);
        eprintln!();
    }

    validate_result
}

/// tree-sitter-sqlによって得られた二つのCSTが等価であるかを判定する。
/// 等価であれば true を、そうでなければ false を返す。
fn compare_tree(
    src_str: &str,
    format_result: &str,
    src_ts_tree: &Tree,
    dst_ts_tree: &Tree,
) -> Result<(), UroboroSQLFmtError> {
    compare_node(
        src_str,
        format_result,
        &src_ts_tree.root_node(),
        &dst_ts_tree.root_node(),
    )
}

/// 二つのノードを比較して、等価なら true を、そうでなければ false を返す。
fn compare_node(
    src_str: &str,
    format_result: &str,
    src_node: &Node,
    dst_node: &Node,
) -> Result<(), UroboroSQLFmtError> {
    // TreeCursorでは、2つのCSTを比較しながら走査する処理をきれいに書けなかったため、
    // NodeとNode::children()で実装している。

    if src_node.kind() != dst_node.kind() {
        Err(UroboroSQLFmtError::Validation {
            format_result: format_result.to_owned(),
            error_msg: format!("different kinds. src={:?}, dst={:?}", src_node, dst_node),
        })
    } else {
        compare_leaf(src_str, format_result, src_node, dst_node)?;

        let src_children: Vec<_> = src_node.children(&mut src_node.walk()).collect();
        let dst_children: Vec<_> = dst_node.children(&mut dst_node.walk()).collect();

        let mut src_idx = 0;
        let mut dst_idx = 0;

        while src_idx < src_children.len() && dst_idx < dst_children.len() {
            let src_child = &src_children.get(src_idx).unwrap();
            let dst_child = &dst_children.get(dst_idx).unwrap();

            compare_node(src_str, format_result, src_child, dst_child)?;

            src_idx += 1;
            dst_idx += 1;
        }

        if src_idx != src_children.len() || dst_idx != dst_children.len() {
            return Err(UroboroSQLFmtError::Validation {
                format_result: format_result.to_owned(),
                error_msg: format!(
                    "different children. src={:?}, dst={:?}",
                    src_children, dst_children
                ),
            });
        }

        Ok(())
    }
}

fn compare_leaf(
    src_str: &str,
    format_result: &str,
    src_node: &Node,
    dst_node: &Node,
) -> Result<(), UroboroSQLFmtError> {
    let src_leaf_str = src_node.utf8_text(&src_str.as_bytes()).unwrap();
    let dst_leaf_str = dst_node.utf8_text(&format_result.as_bytes()).unwrap();

    match src_node.kind() {
        COMMENT if src_leaf_str.starts_with("/*+") || src_leaf_str.starts_with("--+") => {
            // ヒント句
            if dst_leaf_str.starts_with("/*+") || dst_leaf_str.starts_with("--+") {
                Ok(())
            } else {
                Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        r#"hint must start with "/*+" or "--+". src={:?}(content={}), dst={:?}(content={})"#,
                        src_node, src_leaf_str, dst_node, dst_leaf_str
                    ),
                })
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use crate::validate::compare_tree;

    #[test]
    fn test_compare_tree_lack_element() {
        let src = r"select column_name as col from table_name";
        let dst = r"select column_name from table_name";

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree).is_err());
    }

    #[test]
    fn test_compare_tree_change_order() {
        let src = r"select * from tbl1,/* comment */ tbl2";
        let dst = r"select * from tbl1/* comment */, tbl2";

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree).is_err());
    }

    #[test]
    fn test_compare_tree_different_children() {
        let src = r"select * from tbl1";
        let dst = r"select * from tbl1, tbl2";

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree).is_err());
    }

    #[test]
    fn test_compare_tree_success() {
        let src = r"
SELECT /*+ optimizer_features_enable('11.1.0.6') */ employee_id, last_name
FROM    employees
ORDER BY employee_id;";

        let dst = r"
SELECT
/*+ optimizer_features_enable('11.1.0.6') */
    employee_id
,   last_name
FROM
    employees
ORDER BY
    employee_id
;";

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree).is_ok());
    }

    #[test]
    fn test_compare_tree_broken_hint() {
        let src = r"
SELECT /*+ optimizer_features_enable('11.1.0.6') */ employee_id, last_name
FROM    employees
ORDER BY employee_id;";

        // /*と+の間に空白・改行が入ってしまっている
        let dst = r"
SELECT
/*
    + optimizer_features_enable('11.1.0.6')
*/
    employee_id
,   last_name
FROM
    employees
ORDER BY
    employee_id
;";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree).is_err());
    }
}
