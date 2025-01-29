use itertools::Itertools;
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::{Node, Tree}, ts_parse};

use crate::{
    config::{load_never_complement_settings, CONFIG}, cst::Location, pg_format_tree, pg_print_cst, util::create_error_annotation, UroboroSQLFmtError
};

/// フォーマット前後でSQLに欠落が生じないかを検証する。
pub(crate) fn validate_format_result(
    src: &str,
) -> Result<(), UroboroSQLFmtError> {
    let src_ts_tree = ts_parse(src, ).unwrap();

    let dbg = CONFIG.read().unwrap().debug;

    // 補完を行わない設定に切り替える
    load_never_complement_settings();

    let format_result = pg_format_tree(&src_ts_tree, src)?;
    let dst_ts_tree = ts_parse(&format_result).unwrap();

    let validate_result = compare_tree(src, &format_result, &src_ts_tree, &dst_ts_tree, src);

    if dbg && validate_result.is_err() {
        eprintln!(
            "\n{} validation error! {}\n",
            "=".repeat(20),
            "=".repeat(20)
        );
        eprintln!("src_ts_tree =");
        pg_print_cst(src_ts_tree.root_node(), 0);
        eprintln!();
        eprintln!("dst_ts_tree =");
        pg_print_cst(dst_ts_tree.root_node(), 0);
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
    src: &str,
) -> Result<(), UroboroSQLFmtError> {
    let mut src_tokens: Vec<Token> = vec![];
    construct_tokens(&src_ts_tree.root_node(), src_str, &mut src_tokens);

    let mut dst_tokens: Vec<Token> = vec![];
    construct_tokens(&dst_ts_tree.root_node(), format_result, &mut dst_tokens);

    swap_comma_and_trailing_comment(&mut src_tokens);

    compare_tokens(&src_tokens, &dst_tokens, format_result, src)
}

#[derive(Debug, PartialEq)]
struct Token {
    kind: SyntaxKind,
    /// すべてテキストを文字列で持つのに問題がある場合、tree_sitter::Node または tree_sitter::Range に変更
    text: String,
    location: Location,
}

impl Token {
    fn new(node: &Node) -> Self {
        let kind = node.kind();
        let text = node.text().into();
        let location = Location::from(node.range());
        Token {
            kind,
            text,
            location,
        }
    }

    fn is_same_kind(&self, other: &Token) -> bool {
        self.kind == other.kind
    }

    /// エラー注釈を作成する関数
    /// 以下の形のエラー注釈を生成
    ///
    /// ```sh
    ///   |
    /// 1 | select * from y y y y y y y y ;
    ///   |                   ^^^^^^^^^^^ After format: "; (kind: ;)"
    ///   |
    /// ```
    fn error_annotation(&self, src: &str, dst_tok: Option<&Token>) -> String {
        let label = if let Some(other) = dst_tok {
            format!(r#"After format: "{} (kind: {})""#, other.text, other.kind)
        } else {
            "".to_string()
        };

        match create_error_annotation(&self.location, &label, src) {
            Ok(error_annotation) => error_annotation,
            Err(_) => "".to_string(),
        }
    }
}

fn construct_tokens(node: &Node, src: &str, tokens: &mut Vec<Token>) {
    if node.child_count() == 0 {
        let token = Token::new(node);
        tokens.push(token);
    } else {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                construct_tokens(&cursor.node(), src, tokens);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }
}

fn compare_tokens(
    src_tokens: &[Token],
    dst_tokens: &[Token],
    format_result: &str,
    src: &str,
) -> Result<(), UroboroSQLFmtError> {
    for test in src_tokens.iter().zip_longest(dst_tokens.iter()) {
        match test {
            itertools::EitherOrBoth::Both(src_tok, dst_tok) => {
                if src_tok.is_same_kind(dst_tok) {
                    compare_token_text(src_tok, dst_tok, format_result, src)?
                } else {
                    return Err(UroboroSQLFmtError::Validation {
                        format_result: format_result.to_owned(),
                        error_msg: format!("different kind token: Errors have occurred near the following token\n{}", src_tok.error_annotation(src,  Some(dst_tok))),
                    });
                }
            }
            itertools::EitherOrBoth::Left(src_tok) => {
                // src.len() > dst.len() の場合
                return Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        "different kind token: Errors have occurred near the following token\n{}",
                        src_tok.error_annotation(src, None)
                    ),
                });
            }
            itertools::EitherOrBoth::Right(_) => {
                // src.len() < dst.len() の場合
                return Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!("different kind token: For some reason the number of tokens in the format result has increased\nformat_result: \n{}", format_result),
                });
            }
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
    src: &str,
) -> Result<(), UroboroSQLFmtError> {
    let src_tok_text = &src_tok.text;
    let dst_tok_text = &dst_tok.text;
    match src_tok.kind {
        SyntaxKind::SQL_COMMENT | SyntaxKind::C_COMMENT if src_tok_text.starts_with("/*+") || src_tok_text.starts_with("--+") => {
            // ヒント句
            if dst_tok_text.starts_with("/*+") || dst_tok_text.starts_with("--+") {
                Ok(())
            } else {
                Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        r#"hint must start with "/*+" or "--+".\n{}"#,
                        src_tok.error_annotation(src, Some(dst_tok))
                    ),
                })
            }
        }
        _ => Ok(()),
    }
}

/// トークン列に [カンマ, 行末コメント] の並びがあれば、それを入れ替える関数。
fn swap_comma_and_trailing_comment(tokens: &mut [Token]) {
    for idx in 0..(tokens.len() - 1) {
        let fst_tok = tokens.get(idx).unwrap();

        if fst_tok.kind == SyntaxKind::Comma && idx + 1 < tokens.len() {
            let snd_tok = tokens.get(idx + 1).unwrap();
            if snd_tok.kind == SyntaxKind::SQL_COMMENT {
                tokens.swap(idx, idx + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use postgresql_cst_parser::{syntax_kind::SyntaxKind, ts_parse};
    use super::compare_tree;

    use crate::{
        cst::{Location, Position},
        error::UroboroSQLFmtError,
    };

    use super::{construct_tokens, Token};

    #[test]
    fn test_compare_tree_lack_element() {
        let src = r"select column_name as col from table_name";
        let dst = r"select column_name from table_name";

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree = ts_parse(dst).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src).is_err());
    }

    #[test]
    fn test_compare_tree_change_order() {
        let src = r"select * from tbl1,/* comment */ tbl2";
        let dst = r"select * from tbl1/* comment */, tbl2";

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree = ts_parse(dst).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src).is_err());
    }

    #[test]
    fn test_compare_tree_different_children() {
        let src = r"select * from tbl1";
        let dst = r"select * from tbl1, tbl2";

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree = ts_parse(dst).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src).is_err());
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

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree = ts_parse(dst).unwrap();

        compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src)
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

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree = ts_parse(dst).unwrap();

        assert!(compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src).is_err());
    }

    fn new_token(kind: SyntaxKind, text: impl Into<String>, location: Location) -> Token {
        let kind = kind.into();
        let text = text.into();
        Token {
            kind,
            text,
            location,
        }
    }

    #[test]
    fn test_construct_tokens() {
        let src = r"select column_name as col from table_name";

        let ts_tree = ts_parse(src).unwrap();
        let mut tokens: Vec<Token> = vec![];
        construct_tokens(&ts_tree.root_node(), src, &mut tokens);

        let expected_tokens = vec![
            new_token(
                SyntaxKind::SELECT,
                "select",
                Location {
                    start_position: Position { row: 0, col: 0 },
                    end_position: Position { row: 0, col: 6 },
                }
            ),
            new_token(
                SyntaxKind::IDENT,
                "column_name",
                Location {
                    start_position: Position { row: 0, col: 7 },
                    end_position: Position { row: 0, col: 18 },
                }
            ),
            new_token(
                SyntaxKind::AS,
                "as",
                Location {
                    start_position: Position { row: 0, col: 19 },
                    end_position: Position { row: 0, col: 21 },
                }
            ),
            new_token(
                SyntaxKind::IDENT,
                "col",
                Location {
                    start_position: Position { row: 0, col: 22 },
                    end_position: Position { row: 0, col: 25 },
                }
            ),
            new_token(
                SyntaxKind::FROM,
                "from",
                Location {
                    start_position: Position { row: 0, col: 26 },
                    end_position: Position { row: 0, col: 30 },
                }
            ),
            new_token(
                SyntaxKind::IDENT,
                "table_name",
                Location {
                    start_position: Position { row: 0, col: 31 },
                    end_position: Position { row: 0, col: 41 },
                }
            )
        ];

        assert_eq!(tokens.len(), expected_tokens.len());

        for (i, (actual, expected)) in tokens.iter().zip(expected_tokens.iter()).enumerate() {
            assert_eq!(actual.kind, expected.kind, "Token {}: kind mismatch", i);
            assert_eq!(actual.text, expected.text, "Token {}: text mismatch", i);
            assert_eq!(actual.location, expected.location, "Token {}th: location mismatch", i);
        }
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

        let src_ts_tree = ts_parse(src).unwrap();
        let dst_ts_tree =ts_parse(dst).unwrap();

        compare_tree(src, dst, &src_ts_tree, &dst_ts_tree, src)
    }
}
