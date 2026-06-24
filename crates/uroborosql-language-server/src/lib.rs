mod code_action;
mod configuration;
mod document;
mod formatting;
mod lint;
mod paths;
mod server;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

pub use tower_lsp_server::ClientSocket;
#[cfg(feature = "runtime-tokio")]
use tower_lsp_server::Server;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::{Client, LspService};
use uroborosql_lint::{ConfigStore, Linter};

use crate::configuration::ClientConfig;
use crate::document::DocumentState;
use crate::formatting::FORMAT_SELECTIONS_AS_SQL_METHOD;
use crate::paths::WorkspaceRoot;

const CONFIGURATION_SECTION: &str = "uroborosql-fmt";
const DEFAULT_FMT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[derive(Clone)]
pub struct Backend {
    client: Client,
    linter: Arc<Linter>,
    documents: Arc<RwLock<HashMap<Uri, DocumentState>>>,
    /// Client config fetched per workspace root, so each root resolves its own
    /// `lintConfigurationFilePath`. Also reused as the formatting fallback when a
    /// live config fetch fails.
    workspace_configs: Arc<RwLock<HashMap<PathBuf, ClientConfig>>>,
    /// Lint config stores keyed by the normalized workspace root path.
    /// `None` distinguishes "no `.uroborosqllintrc.json` for this root" from
    /// "this root has not been resolved yet".
    lint_config_stores: Arc<RwLock<HashMap<PathBuf, Option<ConfigStore>>>>,
    /// Normalized workspace roots resolved from `workspaceFolders` (or the
    /// `rootUri` fallback). All config resolution is scoped through these.
    workspace_roots: Arc<RwLock<Vec<WorkspaceRoot>>>,
    supports_dynamic_watched_files: Arc<RwLock<bool>>,
    has_watched_files_registration: Arc<RwLock<bool>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            linter: Arc::new(Linter::new()),
            documents: Arc::new(RwLock::new(HashMap::new())),
            workspace_configs: Arc::new(RwLock::new(HashMap::new())),
            lint_config_stores: Arc::new(RwLock::new(HashMap::new())),
            workspace_roots: Arc::new(RwLock::new(Vec::new())),
            supports_dynamic_watched_files: Arc::new(RwLock::new(false)),
            has_watched_files_registration: Arc::new(RwLock::new(false)),
        }
    }
}

pub fn create_service() -> (LspService<Backend>, ClientSocket) {
    LspService::build(Backend::new)
        .custom_method(
            FORMAT_SELECTIONS_AS_SQL_METHOD,
            Backend::format_selections_as_sql,
        )
        .finish()
}

#[cfg(feature = "runtime-tokio")]
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = create_service();
    Server::new(stdin, stdout, socket).serve(service).await;
}
