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

    /// 明示的に指定された設定値のみから構成されるクライアント設定の JSON 文字列を取得する
    pub(crate) fn client_config_json_explicit_only(&self) -> String {
        let config = self.client_config.read().unwrap().clone();
        let mut value = serde_json::to_value(config).expect("client config must be serializable");

        if let Some(map) = value.as_object_mut() {
            map.retain(|_, v| !v.is_null());
        }

        value.to_string()
    }
}
