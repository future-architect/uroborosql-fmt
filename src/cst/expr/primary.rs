use itertools::Itertools;
use tree_sitter::Node;

use crate::{
    config::CONFIG,
    cst::{Comment, Location, UroboroSQLFmtError},
    util::{tab_size, to_tab_num},
};

use super::to_uppercase_identifier;

/// 識別子、リテラルを表す。
/// また、キーワードは式ではないが、便宜上PrimaryExprとして扱う場合がある。
#[derive(Clone, Debug)]
pub(crate) struct PrimaryExpr {
    elements: Vec<String>,
    loc: Location,
    /// バインドパラメータ
    head_comment: Option<String>,
}

impl PrimaryExpr {
    pub(crate) fn new(element: impl Into<String>, loc: Location) -> PrimaryExpr {
        PrimaryExpr {
            elements: vec![element.into()],
            loc,
            head_comment: None,
        }
    }

    /// tree_sitter::Node から PrimaryExpr を生成する。
    /// キーワードをPrimaryExprとして扱う場合があり、その際はこのメソッドで生成する。
    pub(crate) fn with_node(node: Node, src: &str) -> PrimaryExpr {
        PrimaryExpr::new(
            node.utf8_text(src.as_bytes()).unwrap(),
            Location::new(node.range()),
        )
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    pub(crate) fn last_line_tab_num(&self) -> usize {
        to_tab_num(self.last_line_len())
    }

    pub(crate) fn last_line_len(&self) -> usize {
        // elementsをフォーマットするとき、各要素間に '\t' が挿入される
        //
        // e.g., TAB_SIZE = 4のとき
        // TAB1.NUM: 8文字 = TAB_SIZE * 2 -> tabを足すと長さTAB_SIZE * 2 + TAB_SIZE
        // TAB1.N  : 5文字 = TAB_SIZE * 1 + 1 -> tabを足すと長さTAB_SIZE + TAB_SIZE
        // -- 例外 --
        // N       : 1文字 < TAB_SIZE -> tabを入れると長さTAB_SIZE

        self.elements
            .iter()
            .map(String::len)
            .enumerate()
            .fold(0, |sum, (i, len)| {
                // 最初の要素には、バインドパラメータがつく可能性がある
                let len = match (i, &self.head_comment) {
                    (0, Some(head_comment)) => head_comment.len() + len,
                    _ => len,
                };

                // フォーマット時に、各elemの間にタブ文字が挿入される
                to_tab_num(sum) * tab_size() + len
            })
    }

    pub(crate) fn elements(&self) -> &Vec<String> {
        &self.elements
    }

    /// バインドパラメータをセットする
    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment {
            text: mut comment,
            mut loc,
        } = comment;

        if CONFIG.read().unwrap().trim_bind_param {
            // 1. /*を削除
            // 2. *\を削除
            // 3. 前後の空白文字を削除
            // 4. /* */付与
            comment = format!(
                "/*{}*/",
                comment
                    .trim_start_matches("/*")
                    .trim_end_matches("*/")
                    .trim()
            );
        }

        self.head_comment = Some(comment);
        loc.append(self.loc.clone());
        self.loc = loc;
    }

    /// elementsにelementを追加する
    pub(crate) fn add_element(&mut self, element: &str) {
        self.elements.push(element.to_owned());
    }

    /// PrimaryExprの結合
    pub(crate) fn append(&mut self, primary: PrimaryExpr) {
        self.elements.append(&mut primary.elements().clone())
    }

    /// フォーマット後の文字列に変換する。
    /// 大文字・小文字は to_uppercase_identifier() 関数の結果に依存する。
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        // 文字列リテラル以外の要素を大文字に変換して、出力する文字列を生成する
        let elements_str = self
            .elements
            .iter()
            .map(|elem| to_uppercase_identifier(elem))
            .join("\t");

        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, elements_str)),
            None => Ok(elements_str),
        }
    }
}
