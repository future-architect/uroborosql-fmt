use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tower_lsp_server::lsp_types::request::{Request, WorkspaceConfiguration};
use tower_lsp_server::lsp_types::{ConfigurationItem, MessageType, Uri};
use uroborosql_fmt::config::PartialConfig;
use uroborosql_lint::{ConfigStore, DEFAULT_CONFIG_FILENAME};

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
    pub(crate) async fn fetch_client_config(&self, scope_uri: Option<Uri>) -> Option<ClientConfig> {
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
                return None;
            }
        };

        let Some(received_config) = configs.first().cloned() else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    &format!("{} returned empty result", WorkspaceConfiguration::METHOD),
                )
                .await;
            return None;
        };

        if received_config.is_null() {
            return Some(ClientConfig::default());
        }

        match serde_json::from_value::<ClientConfig>(received_config) {
            Ok(config) => Some(config),
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("failed to parse uroborosql-fmt config: {err}"),
                    )
                    .await;
                None
            }
        }
    }

    /// Fetches the client config for every workspace root and rebuilds the lint
    /// stores from the result.
    ///
    /// Each root is queried with its own `scopeUri`, so a multi-root client can
    /// return a different `lintConfigurationFilePath` per folder.
    pub(crate) async fn refresh_workspace_configs(&self) {
        let roots = self.workspace_roots.read().unwrap().clone();

        let mut configs: HashMap<PathBuf, ClientConfig> = HashMap::new();
        for root in &roots {
            let Some(config) = self.fetch_client_config(Some(root.uri.clone())).await else {
                continue;
            };
            configs.insert(root.path.clone(), config);
        }
        *self.workspace_configs.write().unwrap() = configs;

        self.rebuild_lint_config_stores().await;
    }

    pub(crate) fn cached_workspace_config_for_uri(&self, uri: &Uri) -> ClientConfig {
        self.workspace_dir_for_uri(uri)
            .and_then(|dir| self.workspace_configs.read().unwrap().get(&dir).cloned())
            .unwrap_or_default()
    }

    /// Rebuilds the lint config store for every workspace root from the cached
    /// per-root client config, without issuing new `workspace/configuration`
    /// requests. Used when only the on-disk config files may have changed.
    ///
    /// The `lintConfigurationFilePath` setting (relative or absolute) is
    /// resolved against each root independently so that nested / sibling
    /// workspaces each pick up their own `.uroborosqllintrc.json`.
    pub(crate) async fn rebuild_lint_config_stores(&self) {
        let roots = self.workspace_roots.read().unwrap().clone();
        let configs = self.workspace_configs.read().unwrap().clone();

        let mut stores = HashMap::new();
        for root in roots {
            let lint_config_path = configs
                .get(&root.path)
                .and_then(|config| config.lint_configuration_file_path.clone());
            let resolved_path =
                resolve_config_path(Some(&root.path), lint_config_path, DEFAULT_CONFIG_FILENAME);
            match ConfigStore::try_new(root.path.clone(), resolved_path) {
                Ok(store) => {
                    stores.insert(root.path, store);
                }
                Err(err) => {
                    self.client
                        .log_message(
                            MessageType::WARNING,
                            format!(
                                "failed to load lint config for {}: {err}",
                                root.path.display()
                            ),
                        )
                        .await;
                    stores.insert(root.path, None);
                }
            }
        }
        *self.lint_config_stores.write().unwrap() = stores;
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
