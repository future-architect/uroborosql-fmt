use crate::config::CONFIG;

pub(crate) fn format_keyword(key: &str) -> String {
    CONFIG.read().unwrap().keyword_case.format(key)
}

/// 引数の文字列長をタブ数換算した長さを返す
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
