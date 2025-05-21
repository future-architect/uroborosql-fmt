use itertools::Itertools;
use postgresql_cst_parser::{lex, Token, TokenKind};

use crate::{
    config::load_never_complement_settings, pg_format, pg_format_two_way_sql, UroboroSQLFmtError,
};

/// フォーマット前後でSQLに欠落が生じないかを検証する。
/// is_2way_sql_mode には 2way-sql モードでフォーマットするかどうかを指定する。
pub(crate) fn validate_format_result(
    src: &str,
    is_2way_sql_mode: bool,
) -> Result<(), UroboroSQLFmtError> {
    // 補完を行わない設定に切り替える
    load_never_complement_settings();

    let format_result = if is_2way_sql_mode {
        pg_format_two_way_sql(src)?
    } else {
        pg_format(src)?
    };

    let mut src_tokens = lex(src);
    swap_comma_and_trailing_comment(&mut src_tokens);

    let dst_tokens = lex(&format_result);

    compare_tokens(&src_tokens, &dst_tokens, src, &format_result)
}

fn error_annotation(src_token: &Token, dst_token: Option<&Token>, src: &str) -> String {
    // location の形式が違うのでそのままは使えない
    "wip".to_string()
}

fn compare_tokens(
    src_tokens: &[Token],
    dst_tokens: &[Token],
    src: &str,
    format_result: &str,
) -> Result<(), UroboroSQLFmtError> {
    for test in src_tokens.iter().zip_longest(dst_tokens.iter()) {
        match test {
            itertools::EitherOrBoth::Both(src_token, dst_token) => {
                if src_token.kind == dst_token.kind {
                    compare_token_text(src_token, dst_token, format_result, src)?
                } else {
                    return Err(UroboroSQLFmtError::Validation {
                        format_result: format_result.to_owned(),
                        error_msg: format!("different kind token: Errors have occurred near the following token\n{}", error_annotation(src_token, Some(dst_token), src)),
                    });
                }
            }
            itertools::EitherOrBoth::Left(src_token) => {
                // src.len() > dst.len() の場合
                return Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        "different kind token: Errors have occurred near the following token\n{}",
                        error_annotation(src_token, None, src)
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
    src_token: &Token,
    dst_token: &Token,
    format_result: &str,
    src: &str,
) -> Result<(), UroboroSQLFmtError> {
    let src_token_text = &src_token.value;
    let dst_token_text = &dst_token.value;
    match src_token.kind {
        TokenKind::SQL_COMMENT | TokenKind::C_COMMENT
            if src_token_text.starts_with("/*+") || src_token_text.starts_with("--+") =>
        {
            // ヒント句
            if dst_token_text.starts_with("/*+") || dst_token_text.starts_with("--+") {
                Ok(())
            } else {
                Err(UroboroSQLFmtError::Validation {
                    format_result: format_result.to_owned(),
                    error_msg: format!(
                        r#"hint must start with "/*+" or "--+".\n{}"#,
                        error_annotation(src_token, Some(dst_token), src)
                    ),
                })
            }
        }
        _ => Ok(()),
    }
}

/// [カンマ, 行末コメント] という並びのトークンを入れ替える
fn swap_comma_and_trailing_comment(tokens: &mut [Token]) {
    if tokens.len() < 2 {
        return;
    }

    // 0..(tokens.len() - 1) でループするため i+1 は常に有効なインデックスである
    for i in 0..(tokens.len() - 1) {
        if tokens[i].value == "," && tokens[i + 1].value.starts_with("--") {
            tokens.swap(i, i + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use postgresql_cst_parser::lex;

    use crate::{error::UroboroSQLFmtError, pg_validate::compare_tokens};

    use super::swap_comma_and_trailing_comment;

    #[test]
    fn test_compare_tokens_lack_element() {
        let src = r"select column_name1, column_name2 as col from table_name";
        let dst = r"select column_name1, column_name2 as col from table_name";

        let src_tokens = lex(src);
        let dst_tokens = lex(dst);

        assert!(compare_tokens(&src_tokens, &dst_tokens, src, dst).is_err());
    }

    #[test]
    fn test_compare_tokens_change_order() {
        let src = r"select * from tbl1,/* comment */ tbl2";
        let dst = r"select * from tbl1/* comment */, tbl2";

        let src_tokens = lex(src);
        let dst_tokens = lex(dst);

        assert!(compare_tokens(&src_tokens, &dst_tokens, src, dst).is_err());
    }

    #[test]
    fn test_compare_tokens_different_children() {
        let src = r"select * from tbl1";
        let dst = r"select * from tbl1, tbl2";

        let src_tokens = lex(src);
        let dst_tokens = lex(dst);

        assert!(compare_tokens(&src_tokens, &dst_tokens, src, dst).is_err());
    }

    #[test]
    fn test_compare_tokens_success() -> Result<(), UroboroSQLFmtError> {
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

        let src_tokens = lex(src);
        let dst_tokens = lex(dst);

        compare_tokens(&src_tokens, &dst_tokens, src, dst)
    }

    #[test]
    fn test_compare_tokens_broken_hint() {
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

        let src_tokens = lex(src);
        let dst_tokens = lex(dst);

        assert!(compare_tokens(&src_tokens, &dst_tokens, src, dst).is_err());
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

        let mut src_tokens = lex(src);
        swap_comma_and_trailing_comment(&mut src_tokens);
        let dst_tokens = lex(dst);

        compare_tokens(&src_tokens, &dst_tokens, src, dst)
    }
}
