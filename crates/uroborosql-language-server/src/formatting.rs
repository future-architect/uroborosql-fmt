use std::path::PathBuf;

use serde_json::Value;

use crate::configuration::resolve_config_path;
use crate::{Backend, DEFAULT_FMT_CONFIG_PATH};

const NON_FORMATTER_CONFIG_KEYS: [&str; 2] =
    ["configuration_file_path", "lint_configuration_file_path"];

fn formatter_config_json_explicit_only(value: Value) -> String {
    let mut value = value;

    if let Some(map) = value.as_object_mut() {
        map.retain(|key, value| {
            !value.is_null() && !NON_FORMATTER_CONFIG_KEYS.contains(&key.as_str())
        });
    }

    value.to_string()
}

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

    pub(crate) fn client_config_json_explicit_only(&self) -> String {
        let config = self.client_config.read().unwrap().clone();
        let value = serde_json::to_value(config).expect("client config must be serializable");
        formatter_config_json_explicit_only(value)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::formatter_config_json_explicit_only;

    #[test]
    fn explicit_formatter_config_excludes_path_fields() {
        let config_json = formatter_config_json_explicit_only(json!({
            "keyword_case": "upper",
            "configuration_file_path": "fmt.json",
            "lint_configuration_file_path": "lint.json",
            "tab_size": null
        }));

        let config_value: serde_json::Value =
            serde_json::from_str(&config_json).expect("filtered config must be valid json");

        assert_eq!(config_value, json!({ "keyword_case": "upper" }));
    }
}
