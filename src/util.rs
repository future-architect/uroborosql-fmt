use crate::config::CONFIG;

pub(crate) fn format_keyword(key: &str) -> String {
    CONFIG.read().unwrap().keyword_upper_or_lower.format(key)
}
