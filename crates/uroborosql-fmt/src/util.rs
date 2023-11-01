use annotate_snippets::{
    display_list::{DisplayList, FormatOptions},
    snippet::{AnnotationType, Slice, Snippet, SourceAnnotation},
};
use itertools::Itertools;

use crate::{config::CONFIG, cst::Location, error::UroboroSQLFmtError};

/// 設定ファイルに合わせて予約後の大文字・小文字を変換する
pub(crate) fn convert_keyword_case(keyword: &str) -> String {
    CONFIG.read().unwrap().keyword_case.format(keyword)
}

/// 引数の文字列が識別子であれば設定ファイルに合わせて大文字小文字変換をして返す
/// 文字列リテラル、または引用符付き識別子である場合はそのままの文字列を返す
pub(crate) fn convert_identifier_case(identifier: &str) -> String {
    if is_quoted(identifier) {
        identifier.to_owned()
    } else {
        CONFIG.read().unwrap().identifier_case.format(identifier)
    }
}

/// 引数の文字列が引用符付けされているかどうかを判定する。
/// 引用符付けされている場合は true を返す。
pub(crate) fn is_quoted(elem: &str) -> bool {
    (elem.starts_with('"') && elem.ends_with('"'))
        || (elem.starts_with('\'') && elem.ends_with('\''))
        || (elem.starts_with('$') && elem.ends_with('$'))
}

/// 引数の文字列長をタブ数換算した長さを返す
///
/// 例えばtabsize = 4の場合
///
/// to_tab_num(4) => 8
///
/// to_tab_num(4) => 4
pub(crate) fn to_tab_num(len: usize) -> usize {
    if len == 0 {
        0
    } else {
        len / tab_size() + 1
    }
}

/// 設定からタブ長を取得する
pub(crate) fn tab_size() -> usize {
    CONFIG.read().unwrap().tab_size
}

/// 設定の trim_bind_param が true であるとき、引数のバインドパラメータの空白をトリムして返す。
/// 設定が false であるときは、引数をそのまま返す。
pub(crate) fn trim_bind_param(text: String) -> String {
    if CONFIG.read().unwrap().trim_bind_param {
        // 1. /*を削除
        // 2. *\を削除
        // 3. 前後の空白文字を削除
        // 4. /* */付与
        format!(
            "/*{}*/",
            text.trim_start_matches("/*").trim_end_matches("*/").trim()
        )
    } else {
        text
    }
}

/// 引数が定義ファイルで設定した1行の文字数上限を超えていた場合 true を返す
pub(crate) fn is_line_overflow(char_len: usize) -> bool {
    // 1行当たりの上限文字数
    let max_char_per_line = CONFIG.read().unwrap().max_char_per_line;

    if max_char_per_line < 0 {
        // 負の数値が設定されている場合は改行しない
        false
    } else {
        char_len >= max_char_per_line as usize
    }
}

/// xバイト目が何文字目かを返す
fn byte_to_char_index(input: &str, target_byte_index: usize) -> Result<usize, UroboroSQLFmtError> {
    let mut char_index = 0;
    let mut byte_index = 0;

    for c in input.chars() {
        if byte_index == target_byte_index {
            return Ok(char_index);
        }
        char_index += 1;
        byte_index += c.len_utf8();
    }

    if byte_index == target_byte_index {
        Ok(char_index)
    } else {
        Err(UroboroSQLFmtError::Runtime(format!(
            "byte_to_char_index: byte_index({}) is out of range",
            target_byte_index
        )))
    }
}

/// エラー注釈を作成する関数
/// 以下の形のエラー注釈を生成
///
/// ```sh
///   |
/// 2 | using tbl_b b
///   | ^^^^^^^^^^^^^ {label}
///   |
/// ```
pub(crate) fn create_error_annotation(
    location: &Location,
    label: &str,
    src: &str,
) -> Result<String, UroboroSQLFmtError> {
    // 元のSQLのエラーが発生した行を取得
    let source = src
        .lines()
        .enumerate()
        .filter(|(i, _)| (location.start_position.row..=location.end_position.row).contains(i))
        .map(|(_, x)| x)
        .join("\n");

    // エラー発生箇所の開始位置
    // locaton.start_position.colはバイト数を指しているので文字数に変換する
    let start_point = byte_to_char_index(
        src.lines().collect_vec()[location.start_position.row],
        location.start_position.col,
    )?;

    // エラー発生箇所の終了位置
    // = (終了位置までの行の文字数合計) + (終了位置の行の終了位置までの文字数)
    let end_point = src
        .lines()
        .enumerate()
        .filter(|(i, _)| (location.start_position.row..location.end_position.row).contains(i))
        .map(|(_, x)| x.chars().count() + 1) // 改行コードの分1プラスする
        .sum::<usize>()
        + byte_to_char_index(
            src.lines().collect_vec()[location.end_position.row],
            location.end_position.col,
        )?; // エラー発生行の終了位置までの文字数 (locaton.end_position.colはバイト数を指しているので文字数に変換する)

    let snippet = Snippet {
        title: None,
        footer: vec![],
        slices: vec![Slice {
            source: &source,
            line_start: location.start_position.row + 1,
            origin: None,
            fold: true,
            annotations: vec![SourceAnnotation {
                label,
                annotation_type: AnnotationType::Error,
                range: (start_point, end_point),
            }],
        }],
        opt: FormatOptions {
            color: true,
            ..Default::default()
        },
    };

    Ok(DisplayList::from(snippet).to_string())
}
