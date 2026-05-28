mod code_action;
mod configuration;
mod document;
mod formatting;
mod lint;
mod paths;
mod server;

use std::collections::HashMap;
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

const CONFIGURATION_SECTION: &str = "uroborosql-fmt";
const DEFAULT_FMT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[derive(Clone)]
pub struct Backend {
    client: Client,
    linter: Arc<Linter>,
    documents: Arc<RwLock<HashMap<Uri, DocumentState>>>,
    client_config: Arc<RwLock<ClientConfig>>,
    lint_config_store: Arc<RwLock<Option<ConfigStore>>>,
    root_uri: Arc<RwLock<Option<Uri>>>,
    supports_dynamic_watched_files: Arc<RwLock<bool>>,
    has_watched_files_registration: Arc<RwLock<bool>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            linter: Arc::new(Linter::new()),
            documents: Arc::new(RwLock::new(HashMap::new())),
            client_config: Arc::new(RwLock::new(ClientConfig::default())),
            lint_config_store: Arc::new(RwLock::new(None)),
            root_uri: Arc::new(RwLock::new(None)),
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
