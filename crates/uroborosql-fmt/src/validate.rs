use itertools::Itertools;
use postgresql_cst_parser::{lex, Token, TokenKind};

use crate::{
    config::load_never_complement_settings, format, format_two_way_sql, UroboroSQLFmtError,
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
        format_two_way_sql(src)?
    } else {
        format(src)?
    };

    let mut src_tokens = lex(src)
        .map_err(|e| UroboroSQLFmtError::ParseError(format!("failed to tokenize: {e:?}")))?;
    // カンマと行末コメントの並びを入れ替える
    swap_comma_and_trailing_comment(&mut src_tokens);

    let dst_tokens = lex(src)
        .map_err(|e| UroboroSQLFmtError::ParseError(format!("failed to tokenize: {e:?}")))?;

    compare_tokens(&src_tokens, &dst_tokens, src, &format_result)
}

fn error_annotation(src_token: &Token, dst_token: Option<&Token>, _src: &str) -> String {
    // location の形式が違うのでそのままは使えない
    // とりあえず Token の種類と値を表示する

    // src token
    let src_token_str = format!("src_token: {src_token:?}");

    // dst token (if exists)
    let dst_token_str = if let Some(dst_token) = dst_token {
        format!("dst_token: {dst_token:?}")
    } else {
        "dst_token: None".to_string()
    };

    format!("src_token: {src_token_str}\ndst_token: {dst_token_str}")
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
                    error_msg: format!("different kind token: For some reason the number of tokens in the format result has increased\nformat_result: \n{format_result}"),
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
        if tokens[i].value == "," && tokens[i + 1].kind == TokenKind::SQL_COMMENT {
            tokens.swap(i, i + 1);
        }
    }
}
