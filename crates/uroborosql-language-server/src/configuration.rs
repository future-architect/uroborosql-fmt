use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tower_lsp_server::lsp_types::request::{Request, WorkspaceConfiguration};
use tower_lsp_server::lsp_types::{ConfigurationItem, MessageType};
use uroborosql_fmt::config::PartialConfig;
use uroborosql_lint::ConfigStore;

use crate::{Backend, CONFIGURATION_SECTION};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(crate) struct ClientConfig {
    #[serde(flatten)]
    pub formatter: PartialConfig,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "configurationFilePath"
    )]
    pub configuration_file_path: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "lintConfigurationFilePath"
    )]
    pub lint_configuration_file_path: Option<String>,
}

impl Backend {
    pub(crate) async fn refresh_client_config(&self) {
        let scope_uri = self.root_uri.read().unwrap().clone();
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
                        format!("{} failed: {err}", WorkspaceConfiguration::METHOD),
                    )
                    .await;
                return;
            }
        };

        let Some(received_config) = configs.first().cloned() else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    &format!("{} returned empty result", WorkspaceConfiguration::METHOD),
                )
                .await;
            return;
        };

        if received_config.is_null() {
            *self.client_config.write().unwrap() = ClientConfig::default();
            return;
        }

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
        let Some(root_dir) = self.root_dir() else {
            return;
        };

        let resolved_path = self.resolve_lint_config_path();
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

pub(crate) fn resolve_config_path(
    root_dir: Option<&Path>,
    raw_path: Option<String>,
    default_filename: &str,
) -> Option<PathBuf> {
    let root_dir = root_dir?;

    if let Some(path_string) = raw_path.filter(|s| !s.is_empty()) {
        let path = PathBuf::from(path_string);
        return if path.is_absolute() {
            Some(path)
        } else {
            Some(root_dir.join(path))
        };
    }

    let path = root_dir.join(default_filename);
    path.exists().then_some(path)
}
