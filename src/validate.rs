use tree_sitter::{Language, Node, Tree};

use crate::{
    config::{load_never_complement_settings, CONFIG},
    format, print_cst,
    two_way_sql::format_two_way_sql,
    visitor::COMMENT,
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

/// tree-sitter-sqlによって得られた二つのCSTをトークン列に変形させ、それらを比較して等価であるかを判定する。
/// 等価であれば true を、そうでなければ false を返す。
fn compare_tree(
    src_str: &str,
    format_result: &str,
    src_ts_tree: &Tree,
    dst_ts_tree: &Tree,
) -> Result<(), UroboroSQLFmtError> {
    let mut src_tokens: Vec<Token> = vec![];
    construct_tokens(&src_ts_tree.root_node(), src_str, &mut src_tokens);

    let mut dst_tokens: Vec<Token> = vec![];
    construct_tokens(&dst_ts_tree.root_node(), format_result, &mut dst_tokens);

    swap_comma_and_trailing_comment(&mut src_tokens);

    compare_tokens(&src_tokens, &dst_tokens, format_result)
}

#[derive(Debug, PartialEq)]
struct Token {
    kind: String,
    /// すべてテキストを文字列で持つのに問題がある場合、tree_sitter::Node または tree_sitter::Range に変更
    text: String,
}

impl Token {
    fn new(node: &Node, src: &str) -> Self {
        let kind = node.kind().to_owned();
        let text = node.utf8_text(src.as_bytes()).unwrap().to_owned();
        Token { kind, text }
    }

    fn is_same_kind(&self, other: &Token) -> bool {
        self.kind == other.kind
    }
}

fn construct_tokens(node: &Node, src: &str, tokens: &mut Vec<Token>) {
    if node.child_count() == 0 {
        // leaf
        let token = Token::new(node, src);
        tokens.push(token);
    } else {
        let children: Vec<_> = node.children(&mut node.walk()).collect();
        for child_node in children {
            construct_tokens(&child_node, src, tokens);
        }
    }
}

fn compare_tokens(
    src_tokens: &Vec<Token>,
    dst_tokens: &Vec<Token>,
    format_result: &str,
) -> Result<(), UroboroSQLFmtError> {
    // トークン列の長さの違いは前処理で対処することを想定する。
    // 対処しきれない場合、この関数を変更する。
    if src_tokens.len() != dst_tokens.len() {
        return Err(UroboroSQLFmtError::Validation {
            format_result: format_result.to_owned(),
            error_msg: format!(
                "different length. src={}, dst={}",
                src_tokens.len(),
                dst_tokens.len()
            ),
        });
    }

    for (src_tok, dst_tok) in src_tokens.iter().zip(dst_tokens.iter()) {
        if src_tok.is_same_kind(dst_tok) {
            compare_token_text(src_tok, dst_tok, format_result)?
        } else {
            return Err(UroboroSQLFmtError::Validation {
                format_result: format_result.to_owned(),
                error_msg: format!("different kind token: src={:?}, dst={:?}", src_tok, dst_tok),
            });
        }
    }

    Ok(())
}

/// トークンのテキストを比較する関数。
/// src_tok と dst_tok の kind は等しいことを想定している。
/// 現状は、ヒント句が正しく変形されているかのみを検証する。
fn compare_token_text(
    src_tok: &Token,
    dst_tok: &Token,
    format_result: &str,
) -> Result<(), UroboroSQLFmtError> {
    let src_tok_text = &src_tok.text;
    let dst_tok_text = &dst_tok.text;
    match src_tok.kind.as_str() {
        COMMENT if src_tok_text.starts_with("/*+") || src_tok_text.starts_with("--+") => {
            // ヒント句
            if dst_tok_text.starts_with("/*+") || dst_tok_text.starts_with("--+") {
                Ok(())
            } else {
                Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        r#"hint must start with "/*+" or "--+". src={:?}, dst={:?}"#,
                        src_tok, dst_tok
                    ),
                })
            }
        }
        _ => Ok(()),
    }
}

/// トークン列に [カンマ, 行末コメント] の並びがあれば、それを入れ替える関数。
fn swap_comma_and_trailing_comment(tokens: &mut Vec<Token>) {
    for idx in 0..(tokens.len() - 1) {
        let fst_tok = tokens.get(idx).unwrap();

        if fst_tok.kind == "," && idx + 1 < tokens.len() {
            let snd_tok = tokens.get(idx + 1).unwrap();
            if snd_tok.kind == COMMENT && snd_tok.text.starts_with("--") {
                tokens.swap(idx, idx + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{error::UroboroSQLFmtError, validate::compare_tree};

    use super::{construct_tokens, Token};

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
    fn test_compare_tree_success() -> Result<(), UroboroSQLFmtError> {
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

        compare_tree(src, dst, &src_ts_tree, &dst_ts_tree)
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

    fn new_token(kind: impl Into<String>, text: impl Into<String>) -> Token {
        let kind = kind.into();
        let text = text.into();
        Token { kind, text }
    }

    #[test]
    fn test_construct_tokens() {
        let src = r"select column_name as col from table_name";
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let ts_tree = parser.parse(src, None).unwrap();

        let mut tokens: Vec<Token> = vec![];
        construct_tokens(&ts_tree.root_node(), src, &mut tokens);

        assert_eq!(
            tokens,
            vec![
                new_token("SELECT", "select"),
                new_token("identifier", "column_name"),
                new_token("AS", "as"),
                new_token("identifier", "col"),
                new_token("FROM", "from"),
                new_token("identifier", "table_name")
            ]
        )
    }

    #[test]
    fn test_swap_comma_and_trailing_comment() -> Result<(), UroboroSQLFmtError> {
        let src = r"
select
    col1, -- comment
    col2";

        let dst = r"
select
    col1 -- comment
,   col2";

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(tree_sitter_sql::language()).unwrap();

        let src_ts_tree = parser.parse(src, None).unwrap();
        let dst_ts_tree = parser.parse(dst, None).unwrap();

        compare_tree(src, dst, &src_ts_tree, &dst_ts_tree)
    }
}
