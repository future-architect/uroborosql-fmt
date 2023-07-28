use itertools::EitherOrBoth::{Both, Left, Right};
use itertools::Itertools;

use crate::error::UroboroSQLFmtError;

use super::dag::{Kind, Dag};

/// DAGから作成したTreeのノード
#[derive(Debug, Clone)]
pub(crate) enum TreeNode {
    /// 子供
    Parent(Vec<TreeNode>),
    /// 実際にソースコードを持つ葉ノード。分岐に対応した文字列全体を持つ。
    Leaf(String),
}

impl TreeNode {
    /// 新規の葉を作成
    fn new_leaf() -> Self {
        TreeNode::Leaf(String::new())
    }

    /// 葉 -> 文字列を先頭に挿入
    /// 親 -> 全ての子に対して`insert_head_str(string)`を実行
    fn insert_head_str(&mut self, string: &str) {
        match self {
            TreeNode::Leaf(source) => source.insert_str(0, string),
            TreeNode::Parent(children) => children
                .iter_mut()
                .for_each(|child| child.insert_head_str(string)),
        }
    }

    /// 葉 -> 文字列を末尾に追加
    /// 親 -> 全ての子に対して`push_str(string)`を実行
    fn push_str(&mut self, string: &str) {
        match self {
            TreeNode::Leaf(source) => source.push_str(string),
            TreeNode::Parent(children) => {
                children.iter_mut().for_each(|child| child.push_str(string))
            }
        }
    }

    /// IFを採用する場合とELSEを採用する場合の結果を追加するメソッド
    fn append_node(
        &mut self,
        if_else_buffer: &mut [TreeNode],
    ) -> Result<TreeNode, UroboroSQLFmtError> {
        self.append_node_rec(&mut TreeNode::Parent(if_else_buffer.to_vec()))
    }

    /// IFを採用する場合とELSEを採用する場合の結果を追加する際に、実際の計算を担当するメソッド
    /// 実装の都合上、if/elseのノードに仮の親をつける
    fn append_node_rec(
        &mut self,
        if_else_node: &mut TreeNode,
    ) -> Result<TreeNode, UroboroSQLFmtError> {
        match (self, if_else_node) {
            // 自身が葉の時は、そのまま子供につければよい
            (TreeNode::Leaf(self_src), TreeNode::Parent(if_else_child)) => {
                if_else_child
                    .iter_mut()
                    .for_each(|r| r.insert_head_str(self_src));
                Ok(TreeNode::Parent(if_else_child.to_vec()))
            }
            // 両方が親の時は、子供ごとに結合
            (TreeNode::Parent(self_child), TreeNode::Parent(if_else_child)) => {
                // 1つ前に出現した自身の子ノード
                let mut pre_tree_node: Option<TreeNode> = None;

                // zip_longestで、異なる長さのiteratorをzipできる
                let zipped_children =                     
                self_child
                .iter()
                .zip_longest(if_else_child.iter_mut())
                .map(|pair| match pair {
                    Both(l, r) => {
                        // 現在のノードをpre_tree_nodeとして記憶
                        pre_tree_node = Some(l.clone());
                        l.clone().append_node_rec(r)
                    }
                    // 自身の方が子が多い場合、そのまま子を返す
                    Left(l) => Ok(l.clone()),
                    // 結合相手の方が子が多い場合、1つ前に出現した自身のノードと結合する
                    Right(r) => {
                        if let Some(pre) = &pre_tree_node {
                            pre.clone().append_node_rec(r)
                        } else {
                            Err(UroboroSQLFmtError::Runtime("TreeNode::append_node_rec(): Cannot be merge with a parent which has no children".to_string()))
                        }
                    }
                }).collect::<Result<Vec<_>, UroboroSQLFmtError>>()?;

                Ok(TreeNode::Parent(zipped_children))
            }
            (TreeNode::Parent(children), TreeNode::Leaf(if_else_src)) => {
                children
                    .iter_mut()
                    .for_each(|child| child.push_str(if_else_src));
                Ok(TreeNode::Parent(children.clone()))
            }
            (TreeNode::Leaf(self_src), TreeNode::Leaf(if_else_src)) => {
                Ok(TreeNode::Leaf(self_src.to_owned() + if_else_src.as_str()))
            }
        }
    }

    /// Treeをvecに変換 (デバッグ用)
    pub(crate) fn to_vec_string(&self) -> Vec<String> {
        match self {
            TreeNode::Leaf(string) => vec![string.clone()],
            TreeNode::Parent(children) => children
                .iter()
                .flat_map(|child| child.to_vec_string())
                .collect(),
        }
    }
}

/// DAGを走査して条件分岐に対応した木構造を生成する。
/// 戻り値として、結果の木と(if分岐を評価中であれば)ENDノードのidの組を返す。
/// if分岐評価中でなければ、戻り値の第二要素は0を入れる。
///
/// 以下のような処理をしている
/// 1. 現在見ているノード(最初はnode_idに対応するノード)のテキストを結果の末尾に結合
/// 2. 現在のノードに子供がいるなら、それぞれ次のように処理
///     1. 子供がENDのみの場合 -> 分岐の末尾まで進んだことを意味するので、結果とENDのidを返して終了
///     2. 子供がPLAINのみの場合 -> 通常のテキストなので、子供を現在のノードとして、1に戻る
///     3. 子供がIF/ELSE分岐の場合
///         -> 各子供に対して traverse() を再帰的に呼び出し、ENDの手前まで走査を進める。
///         結果に子供の結果を結合し、ENDノードを現在のノードとして、1に戻る
/// 3. 子供がいなくなったら、結果を返して終了   
fn traverse(dag: &Dag, node_id: usize) -> Result<(TreeNode, usize), UroboroSQLFmtError> {
    let mut current_node = dag.get(&node_id)?;
    let mut result = TreeNode::new_leaf();

    // 子供がなくなるまでループさせる
    // if 分岐しているときは、ENDノードが子供の時にreturnする
    loop {
        result.push_str(&current_node.src);
        let children = current_node.children.iter().collect_vec();

        match &children.len() {
            0 => {
                // 子供がいないことは、DAG全体を走査したことを意味する
                break;
            }
            1 => {
                // len() == 1 より、子供は一つであることが保証されている
                let child_id = children.first().unwrap();
                let child_node = dag.get(child_id)?;
                match child_node.kind {
                    Kind::End => {
                        // 分岐終了
                        return Ok((result, **child_id));
                    }
                    Kind::Plain => {
                        current_node = child_node;
                        continue;
                    }
                    Kind::If | Kind::Begin => {
                        let (child_result, end_id) = traverse(dag, **child_id)?;
                        let mut children_results = vec![child_result];
                        result = result.append_node(&mut children_results)?;
                        current_node = dag.get(&end_id)?;
                    }
                    Kind::Else | Kind::Elif => {
                        // else、elifに兄弟がいないことはない
                        return Err(UroboroSQLFmtError::Runtime("traverse: unreachable error".to_string()));
                    }
                }
            }
            _ => {
                // 複数子供がいるため、必ず分岐している
                let mut children_results: Vec<TreeNode> = vec![];
                let mut end_id = None;
                for child_id in children {
                    let (child_result, _end_id) = traverse(dag, *child_id)?;
                    children_results.push(child_result);
                    end_id = Some(_end_id);
                }
                // 子供がすべて END の手前まで進んだら、合流する
                if let Some(end_id) = end_id {
                    result = result.append_node(&mut children_results)?;
                    current_node = dag.get(&end_id)?;
                } else {
                    // 複数子供がいるため、end_id == Noneであることはない
                    unreachable!()
                }
            }
        }
    }

    Ok((result, 0))
}

/// DAGからTreeを作成する
pub(crate) fn generate_tree_from_dag(dag: &Dag) -> Result<TreeNode, UroboroSQLFmtError> {
    // 親 (id = 0と仮定) から始める
    Ok(traverse(dag, 0)?.0)
}
