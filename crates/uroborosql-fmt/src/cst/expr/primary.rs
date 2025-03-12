use tree_sitter::Node;

use crate::{
    cst::{Comment, Location},
    error::UroboroSQLFmtError,
    util::{
        convert_identifier_case, convert_keyword_case, count_width, is_quoted, trim_bind_param,
    },
};

/// PrimaryExprがKeywordかExprか示すEnum
#[derive(Clone, Debug)]
pub(crate) enum PrimaryExprKind {
    Expr,
    Keyword,
}

/// 識別子、リテラルを表す。
/// また、キーワードは式ではないが、便宜上PrimaryExprとして扱う場合がある。
#[derive(Clone, Debug)]
pub(crate) struct PrimaryExpr {
    element: String,
    loc: Location,
    /// バインドパラメータ
    head_comment: Option<String>,
}

impl PrimaryExpr {
    pub(crate) fn new(element: impl Into<String>, loc: Location) -> PrimaryExpr {
        PrimaryExpr {
            element: element.into(),
            loc,
            head_comment: None,
        }
    }

    /// tree_sitter::Node から PrimaryExpr を生成する。
    /// キーワードをPrimaryExprとして扱う場合があり、その際はこのメソッドで生成する。
    /// kindによって自動でキーワードの大文字小文字ルールを適用する
    pub(crate) fn with_node(node: Node, src: &str, kind: PrimaryExprKind) -> PrimaryExpr {
        let element = node.utf8_text(src.as_bytes()).unwrap();

        // PrimaryExprKindによって適用するルールを変更する
        let converted_element = if matches!(kind, PrimaryExprKind::Keyword) {
            // キーワードの大文字小文字設定を適用した文字列
            convert_keyword_case(element)
        } else {
            // 文字列リテラルであればそのまま、DBオブジェクトであれば大文字小文字設定を適用した文字列
            convert_identifier_case(element)
        };

        PrimaryExpr::new(converted_element, Location::new(node.range()))
    }

    pub(crate) fn with_pg_node(
        node: postgresql_cst_parser::tree_sitter::Node,
        expr_kind: PrimaryExprKind,
    ) -> Result<PrimaryExpr, UroboroSQLFmtError> {
        let element = node.text();

        // PrimaryExprKindによって適用するルールを変更する
        let converted_element = if matches!(expr_kind, PrimaryExprKind::Keyword) {
            // キーワードの大文字小文字設定を適用した文字列
            convert_keyword_case(element)
        } else {
            // 文字列リテラルであればそのまま、DBオブジェクトであれば大文字小文字設定を適用した文字列
            convert_identifier_case(element)
        };

        Ok(PrimaryExpr::new(
            converted_element,
            Location::from(node.range()),
        ))
    }

    pub(crate) fn loc(&self) -> Location {
        self.loc.clone()
    }

    /// 自身を描画した際に、最後の行のインデントからの文字列の長さを返す。
    /// 引数 acc には、自身の左側に存在する式のインデントからの長さを与える。
    pub(crate) fn last_line_len_from_left(&self, acc: usize) -> usize {
        // 基本的には日本語の幅を意識しないといけない箇所はここだけだと思われるので
        // ここだけ count_width で長さを計算している
        let mut len = count_width(&self.element) + acc;
        if let Some(head_comment) = &self.head_comment {
            len += count_width(head_comment);
        };
        len
    }

    pub(crate) fn element(&self) -> &str {
        &self.element
    }

    /// 式が識別子であるかどうかを返す。
    /// 識別子である場合は true そうでない場合、false を返す。
    pub(crate) fn is_identifier(&self) -> bool {
        let is_quoted = is_quoted(&self.element);
        let is_num = self.element.parse::<i64>().is_ok();

        !is_quoted && !is_num
    }

    /// バインドパラメータをセットする
    pub(crate) fn set_head_comment(&mut self, comment: Comment) {
        let Comment { text, mut loc } = comment;

        let text = trim_bind_param(text);

        self.head_comment = Some(text);
        loc.append(self.loc.clone());
        self.loc = loc;
    }

    /// フォーマット後の文字列に変換する。
    /// 大文字・小文字は to_uppercase_identifier() 関数の結果に依存する。
    pub(crate) fn render(&self) -> Result<String, UroboroSQLFmtError> {
        match self.head_comment.as_ref() {
            Some(comment) => Ok(format!("{}{}", comment, self.element)),
            None => Ok(self.element.clone()),
        }
    }
}
