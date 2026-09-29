//! The mutex serializes state transitions with notification transport insertion.
//! Client notification futures finish after tx.send, not after a client response.
use crate::{
    Backend,
    configuration::ClientConfig,
    lint::{to_lsp_diagnostic, to_parse_error},
    paths::file_uri_to_path,
};
use ropey::Rope;
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tokio::task::JoinHandle;
use tower_lsp_server::lsp_types::{MessageType, Uri};
use uroborosql_lint::{CatalogReport, ConfigStore, ResolvedLintConfig};

#[derive(Default)]
pub(crate) struct State {
    pub stopped: bool,
    serial: u64,
    pub docs: HashMap<Uri, Document>,
    pub roots: HashMap<PathBuf, Root>,
    pub tasks: Vec<JoinHandle<()>>,
}
#[derive(Default)]
pub(crate) struct Document {
    pub open: bool,
    pub epoch: u64,
    pub request: u64,
    pub active: bool,
    pending: Option<Job>,
}
#[derive(Default)]
pub(crate) struct Root {
    pub generation: u64,
    pub dirty: u64,
    pub fetching: bool,
    pub pending: bool,
    pub config: Option<ClientConfig>,
    pub store: Option<ConfigStore>,
    pub unavailable: bool,
}
struct Job {
    uri: Uri,
    epoch: u64,
    request: u64,
    version: i32,
    root: Option<(PathBuf, u64, u64)>,
    sql: String,
    rope: Rope,
    config: Option<ResolvedLintConfig>,
}
impl State {
    pub fn next(&mut self) -> u64 {
        self.serial = self
            .serial
            .checked_add(1)
            .expect("analysis sequence exhausted");
        self.serial
    }
    pub fn invalidate(&mut self, uri: &Uri) {
        let id = self.next();
        if let Some(doc) = self.docs.get_mut(uri) {
            doc.request = id;
            doc.pending = None;
        }
    }
    pub fn opened(&mut self, uri: &Uri) {
        let id = self.next();
        let doc = self.docs.entry(uri.clone()).or_default();
        doc.open = true;
        doc.epoch = id;
        doc.request = id;
        doc.pending = None;
    }
    fn current(&self, job: &Job) -> bool {
        !self.stopped
            && self
                .docs
                .get(&job.uri)
                .is_some_and(|d| d.open && d.epoch == job.epoch && d.request == job.request)
            && job.root.as_ref().is_none_or(|(p, g, d)| {
                self.roots.get(p).is_some_and(|r| {
                    !r.pending && !r.unavailable && r.generation == *g && r.dirty == *d
                })
            })
    }
}
impl Backend {
    pub(crate) async fn publish_analysis(
        &self,
        uri: Uri,
        diagnostics: Vec<tower_lsp_server::lsp_types::Diagnostic>,
        version: Option<i32>,
    ) {
        let mut stopping = self.stopping.subscribe();
        if *stopping.borrow() {
            return;
        }
        tokio::select! { biased;
            _=stopping.changed()=>{},
            _=self.client.publish_diagnostics(uri,diagnostics,version)=>{},
        }
    }
    pub(crate) async fn log_analysis(&self, level: MessageType, message: String) {
        let mut stopping = self.stopping.subscribe();
        if *stopping.borrow() {
            return;
        }
        tokio::select! { biased;
            _=stopping.changed()=>{},
            _=self.client.log_message(level,message)=>{},
        }
    }

    pub(crate) fn queue_analysis(&self, state: &mut State, uri: &Uri) {
        if state.stopped || !state.docs.get(uri).is_some_and(|d| d.open) {
            return;
        }
        let Some(rope) = self.document_rope(uri) else {
            return;
        };
        let version = self
            .documents
            .read()
            .unwrap()
            .get(uri)
            .map(|d| d.version)
            .unwrap_or_default();
        let mut root_stamp = None;
        let mut config = None;
        if let Some(workspace) = self.workspace_root_for_uri(uri) {
            let Some(root) = state.roots.get(&workspace.path) else {
                return;
            };
            if root.pending || root.unavailable {
                return;
            }
            root_stamp = Some((workspace.path, root.generation, root.dirty));
            if let (Some(store), Some(path)) = (&root.store, file_uri_to_path(uri)) {
                if !crate::paths::has_parent_dir_component(&path) && !store.is_ignored(&path) {
                    config = Some(store.resolve(&path));
                }
            }
        }
        let request = state.next();
        let doc = state.docs.get_mut(uri).unwrap();
        doc.request = request;
        doc.pending = Some(Job {
            uri: uri.clone(),
            epoch: doc.epoch,
            request,
            version,
            root: root_stamp,
            sql: rope.to_string(),
            rope,
            config,
        });
        if !doc.active {
            doc.active = true;
            let backend = self.clone();
            let uri = uri.clone();
            state.tasks.retain(|h| !h.is_finished());
            state.tasks.push(tokio::spawn(async move {
                backend.analysis_worker(uri).await;
            }));
        }
    }
    async fn analysis_worker(&self, uri: Uri) {
        loop {
            let job = {
                let mut state = self.analysis.lock().await;
                let doc = state.docs.get_mut(&uri).unwrap();
                match doc.pending.take() {
                    Some(job) => job,
                    None => {
                        doc.active = false;
                        return;
                    }
                }
            };
            let permit = if job.config.is_some() {
                match tokio::time::timeout(Duration::from_secs(10), self.slots.acquire()).await {
                    Ok(Ok(p)) => Some(p),
                    _ => {
                        let state = self.analysis.lock().await;
                        if state.current(&job) {
                            self.log_analysis(
                                MessageType::INFO,
                                format!(
                                    "{}: catalog analysis deferred: acquisition slots busy",
                                    job.uri.as_str()
                                ),
                            )
                            .await;
                        }
                        continue;
                    }
                }
            } else {
                None
            };
            if !self.analysis.lock().await.current(&job) {
                continue;
            }
            let mut status = None;
            let diagnostics = if let Some(config) = &job.config {
                let provider = config.catalog_provider();
                match self
                    .linter
                    .run_async(&job.sql, config, provider.as_deref())
                    .await
                {
                    Ok(result) => {
                        let failed = result.catalog.has_failures();
                        let summary = match &result.catalog {
                            CatalogReport::Skipped(reason) => {
                                format!("catalog skipped: {reason:?}")
                            }
                            CatalogReport::Statements(statements) => {
                                use uroborosql_lint::catalog::AnalysisStatus;
                                let complete = statements
                                    .iter()
                                    .filter(|s| matches!(s.status, AnalysisStatus::Complete))
                                    .count();
                                let excluded = statements
                                    .iter()
                                    .filter(|s| matches!(s.status, AnalysisStatus::Excluded(_)))
                                    .count();
                                let failures = statements.len() - complete - excluded;
                                // Structured enums only: never connection strings, SQL, or server errors.
                                let reasons: Vec<_> =
                                    statements.iter().filter_map(|s| s.exclusion).collect();
                                format!(
                                    "catalog complete={complete}, excluded={excluded}, failed={failures}, exclusions={reasons:?}"
                                )
                            }
                        };
                        status = Some((
                            if failed {
                                MessageType::ERROR
                            } else {
                                MessageType::INFO
                            },
                            summary,
                        ));
                        result
                            .diagnostics
                            .into_iter()
                            .map(|d| to_lsp_diagnostic(d, Some(&job.rope)))
                            .collect()
                    }
                    Err(e) => vec![to_parse_error(e, Some(&job.rope))],
                }
            } else {
                Vec::new()
            };
            drop(permit);
            let state = self.analysis.lock().await;
            if state.current(&job) {
                self.publish_analysis(job.uri.clone(), diagnostics, Some(job.version))
                    .await;
                if let Some((level, summary)) = status {
                    self.log_analysis(level, format!("{}: {summary}", job.uri.as_str()))
                        .await;
                }
            }
        }
    }
    pub(crate) async fn stop_analysis(&self) {
        self.stopping.send_replace(true);
        let handles = {
            let mut state = self.analysis.lock().await;
            state.stopped = true;
            for doc in state.docs.values_mut() {
                doc.pending = None;
            }
            std::mem::take(&mut state.tasks)
        };
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        for mut handle in handles {
            if tokio::time::timeout_at(deadline, &mut handle)
                .await
                .is_err()
            {
                handle.abort();
                let _ = handle.await;
            }
        }
    }
}
