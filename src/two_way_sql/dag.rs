use std::collections::HashMap;

use indexmap::{indexset, IndexSet};

use crate::cst::UroboroSQLFmtError;
use crate::re::RE;

/// DAGノードの種類
#[derive(Debug, Clone)]
pub(crate) enum Kind {
    /// 通常のテキスト
    Plain,

    /// /*IF ...*/から/*ELSE*/または/*END*/の上までに対応するノード
    If,

    /// /*ELIF*/から/*ELSE*/または/*END*/の上までに対応するノード
    Elif,

    /// /*ELSE*/から/*END*/の上までに対応するノード
    Else,

    /// /*END*/に対応するノード
    End,

    /// /*BEGIN*/から/*IF*/, /*BEGIN*/または/*END*/の上までに対応するノード
    Begin,
}

impl Kind {
    /// 引数がどのKindに対応するかを返す
    pub(crate) fn guess_from_str(src: &str) -> Self {
        if src.contains("/*END*/") {
            Self::End
        } else if src.contains("/*ELSE*/") {
            Self::Else
        } else if src.contains("/*BEGIN*/") {
            Self::Begin
        } else if RE.if_re.find(src).is_some() {
            Self::If
        } else if RE.elif_re.find(src).is_some() {
            Self::Elif
        } else {
            Self::Plain
        }
    }
}

/// 2way-sqlのIF分岐を考慮して生成されるDAGのノード
#[derive(Debug, Clone)]
pub(crate) struct DAGNode {
    /// ノードID
    /// 根のノードIDは0
    pub(crate) id: usize,

    /// ノードの種類
    pub(crate) kind: Kind,

    /// ソース文字列
    pub(crate) src: String,

    /// 自身を始点とした辺が接続されているノードのidを持つ集合。
    pub(crate) children: IndexSet<usize>,
}

impl DAGNode {
    pub(crate) fn new(id: usize, kind: Kind, src: &str, children: IndexSet<usize>) -> Self {
        DAGNode {
            id,
            kind,
            src: src.to_string(),
            children,
        }
    }

    /// 自身のsrcに文字列を追加
    fn push_str(&mut self, string: &str) {
        self.src.push_str(string);
    }

    /// 自身の子に`child_id`を追加
    fn add_child(&mut self, child_id: usize) {
        self.children.insert(child_id);
    }
}

/// SQLから生成したDAGのノードを出現順に保持
/// 全てのDAGNodeは未接続
#[derive(Debug, Clone)]
struct DAGNodes {
    nodes: Vec<DAGNode>,
}

impl From<&str> for DAGNodes {
    fn from(src: &str) -> Self {
        let mut res = DAGNodes { nodes: vec![] };

        res.generate(src);

        res
    }
}

impl DAGNodes {
    /// 現在使用できる最小のノードID
    fn current_available_id(&mut self) -> usize {
        self.nodes.len()
    }

    /// nodes[index]
    fn get(&self, index: usize) -> Result<&DAGNode, UroboroSQLFmtError> {
        match self.nodes.get(index) {
            Some(item) => Ok(item),
            None => Err(UroboroSQLFmtError::Runtime(
                "DAGNodes::get(): index out of range".to_string(),
            )),
        }
    }

    /// nodesの要素数
    fn len(&self) -> usize {
        self.nodes.len()
    }

    /// SQLからDAGNodeを作成
    fn generate(&mut self, src: &str) {
        // キーワードの前後で分割
        let splitted_src = Self::split_before_and_after_keyword(src);

        // 空文字列のノードを根とする
        let root_node = DAGNode::new(self.current_available_id(), Kind::Plain, "", indexset![]);
        let mut pre_node: Option<DAGNode> = Some(root_node);

        // IF、PLAINの順に出現した場合はIFのsrcにPLAINのsrcを追加する、などの処理をここで行う
        for current_src in splitted_src {
            let current_kind = Kind::guess_from_str(current_src);

            match &mut pre_node {
                // pre_nodeがある場合
                Some(pre) => match current_kind {
                    Kind::Plain => {
                        pre.push_str(current_src);
                    }
                    _ => {
                        // preをresに追加
                        self.nodes.push(pre.clone());
                        // IF/ELSEノードを作成してpre_nodeとする
                        let node = DAGNode::new(
                            self.current_available_id(),
                            current_kind.clone(),
                            current_src,
                            indexset![],
                        );

                        if matches!(current_kind, Kind::End) {
                            self.nodes.push(node);
                            // ENDの場合はsrcを持たないのでpre_nodeに追加しない
                            pre_node = None;
                        } else {
                            pre_node = Some(node);
                        }
                    }
                },
                // pre_nodeがない場合
                None => {
                    let node = DAGNode::new(
                        self.current_available_id(),
                        current_kind.clone(),
                        current_src,
                        indexset![],
                    );

                    if matches!(current_kind, Kind::End) {
                        // ENDならsrcを持たないのでpre_nodeに追加しない
                        self.nodes.push(node);
                    } else {
                        pre_node = Some(node);
                    }
                }
            }
        }

        // もしpre_nodeがある場合はresに追加
        if let Some(current_node) = pre_node {
            self.nodes.push(current_node);
        }
    }

    /// /\*IF\*/, /\*ELSE IF\*/, /\*ELSE\*/ /\*BEGIN\*/ の前後で分割したVecを返す
    ///
    ///
    /// ```sql
    /// select
    /// /*IF fst*/
    ///     test1
    /// /*ELSE*/
    ///     test2
    /// /*END*/
    /// from
    ///     tbl1
    /// ```
    /// の場合は
    /// ```txt
    ///vec![
    ///    "select\n",
    ///    "/*IF fst*/",
    ///    "\n    test1\n",
    ///    "/*ELSE*/",
    ///    "\n    test2\n",
    ///    "/*END*/",
    ///    "\nfrom\n    tbl1",
    ///]
    /// ```
    /// を返す
    fn split_before_and_after_keyword(src: &str) -> Vec<&str> {
        let mut res = vec![src];
        let mut current_end = 0;

        for m in RE.branching_keyword_re.find_iter(src) {
            let start = m.start();
            let end = m.end();

            // resの最後の要素を取り出す
            let last = res.pop().unwrap();
            // キーワードの手前でsplit
            let spl1 = last.split_at(start - current_end);
            // キーワードの直後でsplit
            let spl2 = spl1.1.split_at(end - start);

            // [キーワードより手前、キーワード、キーワードの後]を追加
            for s in &[spl1.0, spl2.0, spl2.1] {
                if !s.is_empty() {
                    res.push(s);
                }
            }

            current_end = end;
        }

        res
    }
}

pub(crate) struct Dag {
    dag: HashMap<usize, DAGNode>,
}

impl TryFrom<DAGNodes> for Dag {
    type Error = UroboroSQLFmtError;

    /// `DAGNodes`からDAGを作成
    fn try_from(nodes: DAGNodes) -> Result<Self, Self::Error> {
        let mut res = Dag::new();

        res.generate(&nodes, None, &mut 0)?;

        Ok(res)
    }
}

impl Dag {
    fn new() -> Self {
        Dag {
            dag: HashMap::new(),
        }
    }

    /// dag.get(key)を返す
    pub(crate) fn get(&self, key: &usize) -> Result<&DAGNode, UroboroSQLFmtError> {
        match self.dag.get(key) {
            Some(value) => Ok(value),
            None => Err(UroboroSQLFmtError::Runtime(
                "DAG::get(): key not included in dag".to_string(),
            )),
        }
    }

    /// ノードをdagに追加
    fn add_node(&mut self, node: DAGNode) {
        self.dag.insert(node.id, node);
    }

    /// parent_idの子にchild_idを追加する
    /// parent_idがdagに存在しない場合はErr
    fn add_child_to_parent(
        &mut self,
        parent_id: usize,
        child_id: usize,
    ) -> Result<(), UroboroSQLFmtError> {
        match self.dag.get_mut(&parent_id) {
            Some(value) => {
                value.add_child(child_id);
                Ok(())
            }
            None => Err(UroboroSQLFmtError::Runtime(
                "DAG::add_child_to_parent(): key not included in dag".to_string(),
            )),
        }
    }

    /// DAGNodesからDAGを作成
    /// parent_id以下を探索して木を生成する
    /// 関数終了時、cursorは探索した最後のノードを指す
    fn generate(
        &mut self,
        nodes: &DAGNodes,
        parent_id: Option<usize>,
        cursor: &mut usize,
    ) -> Result<(), UroboroSQLFmtError> {
        let mut parent_id = parent_id;

        // IF/ELIF/ELSE探索中の兄弟ノード
        let mut siblings_id: Vec<usize> = vec![];

        let mut if_mode = false;

        while *cursor < nodes.len() {
            let current_node = nodes.get(*cursor)?;

            match current_node.kind {
                Kind::Plain => {
                    match parent_id {
                        // 親がいない、つまりDAGが初期状態の場合
                        None => {
                            // 現在のPLAINノードをDAGに追加
                            self.add_node(current_node.clone());

                            parent_id = Some(current_node.id);
                        }
                        Some(par_id) => {
                            // 親に自身を子として追加
                            self.add_child_to_parent(par_id, current_node.id)?;

                            // 現在のノードを追加
                            self.add_node(current_node.clone());

                            parent_id = Some(current_node.id);
                        }
                    }
                }
                Kind::If | Kind::Begin | Kind::Else | Kind::Elif => {
                    // もしIFモードでない状態でELSE、ELIFが出現した場合
                    // 1つ外のELSE, ELIFなのでbreak;
                    // このときcursorを1つ戻す
                    if !if_mode
                        && (matches!(current_node.kind, Kind::Else)
                            || matches!(current_node.kind, Kind::Elif))
                    {
                        *cursor -= 1;
                        break;
                    }

                    // dagにノードを追加
                    self.add_node(current_node.clone());

                    // IFモードに変更
                    if_mode = true;

                    // 次のノードに移動
                    *cursor += 1;

                    // 自身の子を探索して木を生成
                    self.generate(nodes, Some(current_node.id), cursor)?;

                    match parent_id {
                        Some(par_id) => {
                            // 親に自身を子として追加
                            self.add_child_to_parent(par_id, current_node.id)?;

                            siblings_id.push(nodes.get(*cursor)?.id);
                        }
                        None => {
                            // IFの親がない場合はあり得ない
                            unreachable!()
                        }
                    }
                }
                Kind::End => {
                    // 1つ外のENDなのでbreak;
                    // このときcursorを1つ戻す
                    if !if_mode {
                        *cursor -= 1;
                        break;
                    }

                    // dagにノードを追加
                    self.dag.insert(current_node.id, current_node.clone());

                    // ifモードを終了
                    if_mode = false;

                    // すべてのIF/ELSEの子にENDノードをを追加
                    for sibling_id in siblings_id {
                        self.add_child_to_parent(sibling_id, current_node.id)?;
                    }

                    // 兄弟ノードのリセット
                    siblings_id = vec![];

                    // ENDノードを親ノードとする
                    parent_id = Some(current_node.id);
                }
            }

            // カーソルを1つ進める
            *cursor += 1;
        }

        Ok(())
    }
}

/// SQLからDAGを作成する
pub(crate) fn generate_dag(src: &str) -> Result<Dag, UroboroSQLFmtError> {
    let nodes = DAGNodes::from(src);

    let dag = Dag::try_from(nodes)?;

    Ok(dag)
}
