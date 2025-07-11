mod dag;
pub(crate) mod merge;
pub(crate) mod tree;

use crate::{config::CONFIG, error::UroboroSQLFmtError, format, re::RE};

use self::{
    dag::generate_dag,
    merge::merge_tree,
    tree::{generate_tree_from_dag, TreeNode},
};

/// 2way-sqlのIF分岐を考慮して発生しうる複数のSQLからなるTreeを作成
pub(crate) fn generate_tree(src: &str) -> Result<TreeNode, UroboroSQLFmtError> {
    // DAGの生成
    let dag = generate_dag(src)?;

    // DAGから木の生成
    let tree = generate_tree_from_dag(&dag)?;

    Ok(tree)
}

/// 引数のsrcが2way-sqlであるかどうか判断
/// 現状`/*IF ...*/`が存在すればtrueを返す
pub(crate) fn is_two_way_sql(src: &str) -> bool {
    RE.if_re.find(src).is_some()
}

/// Treeの全ての葉をフォーマット
fn format_tree(tree: TreeNode) -> Result<TreeNode, UroboroSQLFmtError> {
    match tree {
        TreeNode::Parent(nodes) => {
            let mut childs = vec![];

            for node in nodes {
                childs.push(format_tree(node)?);
            }

            Ok(TreeNode::Parent(childs))
        }
        TreeNode::Leaf(src) => {
            let res = format(&src)?;

            Ok(TreeNode::Leaf(res))
        }
    }
}

/// 2way-sqlをフォーマット（postgresql-cst-parser）
pub(crate) fn format_two_way_sql(src: &str) -> Result<String, UroboroSQLFmtError> {
    // 2way-sqlをIF分岐によって複数SQLへ分割
    let tree = generate_tree(src)?;

    // treeの葉の全てのSQLをフォーマット
    let formatted_tree = format_tree(tree)?;

    if CONFIG.read().unwrap().debug {
        eprintln!("{}", "-".repeat(100));

        for source in formatted_tree.to_vec_string() {
            eprintln!("{source}");
            eprintln!("{}", "-".repeat(100));
        }
    }

    // 各SQLをマージ
    let res = merge_tree(formatted_tree)?;

    Ok(res)
}
