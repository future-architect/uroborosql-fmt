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

    /// Start independent root refreshes; a slow client never holds other roots.
    pub(crate) async fn refresh_workspace_configs(&self) {
        let mut state = self.analysis.lock().await;
        if state.stopped {
            return;
        }
        let roots = self.workspace_roots.read().unwrap().clone();
        let mut completions = Vec::new();
        state
            .roots
            .retain(|p, _| roots.iter().any(|r| &r.path == p));
        for uri in self.open_documents().into_iter().map(|d| d.0) {
            state.invalidate(&uri);
        }
        for root in roots {
            let generation = state.next();
            let entry = state.roots.entry(root.path.clone()).or_default();
            entry.generation = generation;
            entry.fetching = true;
            entry.pending = true;
            let backend = self.clone();
            state.tasks.retain(|h| !h.is_finished());
            let (done, completion) = tokio::sync::oneshot::channel::<()>();
            completions.push(completion);
            state.tasks.push(tokio::spawn(async move {
                let _done = done;
                let request = backend.client.configuration(vec![ConfigurationItem {
                    scope_uri: Some(root.uri),
                    section: Some(CONFIGURATION_SECTION.into()),
                }]);
                let config =
                    match tokio::time::timeout(std::time::Duration::from_secs(5), request).await {
                        Ok(Ok(values)) => values.first().and_then(|v| {
                            if v.is_null() {
                                Some(ClientConfig::default())
                            } else {
                                serde_json::from_value(v.clone()).ok()
                            }
                        }),
                        _ => None,
                    };
                {
                    let mut state = backend.analysis.lock().await;
                    if state.stopped {
                        return;
                    }
                    let Some(entry) = state
                        .roots
                        .get_mut(&root.path)
                        .filter(|r| r.generation == generation)
                    else {
                        return;
                    };
                    entry.fetching = false;
                    if let Some(config) = config {
                        entry.config = Some(config.clone());
                        backend
                            .workspace_configs
                            .write()
                            .unwrap()
                            .insert(root.path.clone(), config);
                    } else {
                        entry.config = None;
                        backend.fail_root(&mut state, &root.path).await;
                        return;
                    }
                }
                backend.build_root(root.path, generation).await;
            }));
        }
        // Removed roots now have normal outside-workspace empty publication.
        for (uri, _, _) in self.open_documents() {
            if self.workspace_root_for_uri(&uri).is_none() {
                self.queue_analysis(&mut state, &uri);
            }
        }
        drop(state);
        for completion in completions {
            let _ = completion.await;
        }
    }

    pub(crate) fn cached_workspace_config_for_uri(&self, uri: &Uri) -> ClientConfig {
        self.workspace_dir_for_uri(uri)
            .and_then(|dir| self.workspace_configs.read().unwrap().get(&dir).cloned())
            .unwrap_or_default()
    }

    pub(crate) async fn rebuild_lint_config_stores(&self) {
        let mut state = self.analysis.lock().await;
        if state.stopped {
            return;
        }
        for (uri, _, _) in self.open_documents() {
            state.invalidate(&uri);
        }
        let paths: Vec<_> = state.roots.keys().cloned().collect();
        for path in paths {
            let root = state.roots.get_mut(&path).unwrap();
            root.dirty += 1;
            if root.fetching || root.pending {
                continue;
            }
            if root.config.is_none() {
                continue;
            }
            root.pending = true;
            let generation = root.generation;
            let backend = self.clone();
            state.tasks.retain(|h| !h.is_finished());
            state.tasks.push(tokio::spawn(async move {
                backend.build_root(path, generation).await;
            }));
        }
    }

    async fn build_root(&self, path: PathBuf, generation: u64) {
        loop {
            let (dirty, config) = {
                let state = self.analysis.lock().await;
                if state.stopped {
                    return;
                }
                let Some(root) = state
                    .roots
                    .get(&path)
                    .filter(|r| r.generation == generation && !r.fetching)
                else {
                    return;
                };
                let Some(config) = root.config.clone() else {
                    return;
                };
                (root.dirty, config)
            };
            let build_path = path.clone();
            let built = tokio::task::spawn_blocking(move || {
                let resolved = resolve_config_path(
                    Some(&build_path),
                    config.lint_configuration_file_path,
                    DEFAULT_CONFIG_FILENAME,
                );
                ConfigStore::try_new(build_path, resolved)
            })
            .await;
            #[cfg(test)]
            if let Some(hook) = &self.build_hook {
                hook().await;
            }
            let mut state = self.analysis.lock().await;
            if state.stopped {
                return;
            }
            let Some(root) = state
                .roots
                .get_mut(&path)
                .filter(|r| r.generation == generation && !r.fetching)
            else {
                return;
            };
            if root.dirty != dirty {
                continue;
            }
            match built {
                Ok(Ok(store)) => {
                    root.store = store;
                    root.pending = false;
                    root.unavailable = false;
                }
                _ => {
                    self.fail_root(&mut state, &path).await;
                    return;
                }
            }
            for (uri, _, _) in self.open_documents() {
                if self.workspace_dir_for_uri(&uri).as_ref() == Some(&path) {
                    self.queue_analysis(&mut state, &uri);
                }
            }
            return;
        }
    }
    async fn fail_root(&self, state: &mut crate::analysis::State, path: &Path) {
        let root = state.roots.get_mut(path).unwrap();
        root.pending = false;
        root.unavailable = true;
        root.store = None;
        for (uri, _, version) in self.open_documents() {
            if self.workspace_dir_for_uri(&uri).as_deref() == Some(path) {
                state.invalidate(&uri);
                self.publish_analysis(uri, vec![], Some(version)).await;
            }
        }
        self.log_analysis(
            MessageType::ERROR,
            format!("{}: lint configuration unavailable", path.display()),
        )
        .await;
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

#[cfg(test)]
pub(crate) type BuildHook = std::sync::Arc<
    dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync,
>;
