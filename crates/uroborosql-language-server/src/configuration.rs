use std::path::{Path, PathBuf};

use crate::{Backend, CONFIGURATION_SECTION};
use serde::{Deserialize, Serialize};
use tower_lsp_server::lsp_types::{ConfigurationItem, MessageType};
use uroborosql_lint::ConfigStore;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "snake_case", deserialize = "camelCase"))]
/// ref. https://github.com/future-architect/vscode-uroborosql-fmt
pub struct ClientConfig {
    pub debug: Option<bool>,
    pub tab_size: Option<usize>,
    pub complement_alias: Option<bool>,
    pub trim_bind_param: Option<bool>,
    pub keyword_case: Option<String>,
    pub identifier_case: Option<String>,
    pub max_char_per_line: Option<isize>,
    pub complement_outer_keyword: Option<bool>,
    pub complement_column_as_keyword: Option<bool>,
    pub remove_table_as_keyword: Option<bool>,
    pub remove_redundant_nest: Option<bool>,
    pub complement_sql_id: Option<bool>,
    pub convert_double_colon_cast: Option<bool>,
    pub unify_not_equal: Option<bool>,
    pub indent_tab: Option<bool>,
    pub use_parser_error_recovery: Option<bool>,
    pub configuration_file_path: Option<String>,
    pub lint_configuration_file_path: Option<String>,
}

impl Backend {
    pub(crate) async fn refresh_client_config(&self) {
        let scope_uri = self.root_uri.read().unwrap().clone();

        // 現在のスコープに対応する、 `uroborosql-fmt` の設定を取得する
        let request_payload = vec![ConfigurationItem {
            scope_uri,
            section: Some(CONFIGURATION_SECTION.to_string()),
        }];

        let configs = match self.client.configuration(request_payload).await {
            Ok(configs) => configs,
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("workspace/configuration failed: {err}"),
                    )
                    .await;
                return;
            }
        };

        // リクエストに対応する一つの設定値が返ってくる
        let Some(received_config) = configs.first().cloned() else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    "workspace/configuration returned empty result",
                )
                .await;
            return;
        };

        // クライアント設定に上書きする
        match serde_json::from_value::<ClientConfig>(received_config) {
            Ok(config) => {
                *self.client_config.write().unwrap() = config;
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("failed to parse uroborosql-fmt config: {err}"),
                    )
                    .await;
            }
        }
    }

    pub(crate) async fn refresh_lint_config_store(&self) {
        let resolved_path = self.resolve_lint_config_path();
        let root_dir = self.root_dir();

        let Some(root_dir) = root_dir else {
            return;
        };

        match ConfigStore::new(root_dir, resolved_path) {
            Ok(store) => {
                *self.lint_config_store.write().unwrap() = Some(store);
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("failed to load lint config: {err}"),
                    )
                    .await;
                *self.lint_config_store.write().unwrap() = None;
            }
        }
    }
}

pub(crate) fn config_path_is_specified(possible_config_path: &Option<String>) -> bool {
    matches!(possible_config_path, Some(path_string) if !path_string.is_empty())
}

pub(crate) fn resolve_config_path(
    root_dir: Option<&Path>,
    raw_path: Option<String>,
    default_filename: &str,
) -> Option<PathBuf> {
    let root_dir = root_dir?;

    // 設定ファイルが指定されている場合
    if config_path_is_specified(&raw_path) {
        let path = PathBuf::from(raw_path.expect("checked above"));
        if path.is_absolute() {
            Some(path)
        } else {
            Some(root_dir.join(path))
        }
    } else {
        let path = root_dir.join(default_filename);
        if path.exists() { Some(path) } else { None }
    }
}
