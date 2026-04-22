use std::path::PathBuf;

use crate::configuration::resolve_config_path;
use crate::{Backend, DEFAULT_FMT_CONFIG_PATH};

impl Backend {
    pub(crate) fn resolve_fmt_config_path(&self) -> Option<PathBuf> {
        let raw_path = self
            .client_config
            .read()
            .unwrap()
            .configuration_file_path
            .clone();
        let root_dir = self.root_dir();
        resolve_config_path(root_dir.as_deref(), raw_path, DEFAULT_FMT_CONFIG_PATH)
    }

    /// クライアントから受け取った formatter 設定のうち、**明示的に指定された項目だけ** を
    /// JSON 文字列として返す。`format_sql` の `settings_json` 引数にそのまま渡せる。
    ///
    /// `PartialConfig` は全フィールドが `Option<T>` + `skip_serializing_if = "Option::is_none"`
    /// なので、未指定 (None) の項目は自動的に JSON に含まれない。
    /// LSP 固有フィールド (`configuration_file_path` 等) は `ClientConfig.formatter` に
    /// 含まれないため、追加のフィルタも不要。
    pub(crate) fn client_config_json_explicit_only(&self) -> String {
        let formatter = self.client_config.read().unwrap().formatter.clone();
        serde_json::to_string(&formatter).expect("PartialConfig must be serializable")
    }
}

#[cfg(test)]
mod tests {
    use uroborosql_fmt::config::PartialConfig;

    #[test]
    fn explicit_formatter_config_omits_none_and_path_fields() {
        let partial = PartialConfig {
            keyword_case: Some(uroborosql_fmt::config::Case::Upper),
            ..PartialConfig::default()
        };
        let json = serde_json::to_string(&partial).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed, serde_json::json!({ "keyword_case": "upper" }));
    }
}
