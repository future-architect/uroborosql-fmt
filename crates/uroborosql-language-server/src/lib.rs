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

const CONFIGURATION_SECTION: &str = "uroborosql-fmt";
const DEFAULT_FMT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[derive(Clone)]
pub struct Backend {
    client: Client,
    linter: Arc<Linter>,
    documents: Arc<RwLock<HashMap<Uri, DocumentState>>>,
    /// language client の設定
    client_config: Arc<RwLock<ClientConfig>>,
    lint_config_store: Arc<RwLock<Option<ConfigStore>>>,
    // LSP の設定取得など、URI が必要な場面向けに保持する
    root_uri: Arc<RwLock<Option<Uri>>>, // 現状、単一ワークスペースしか考慮していない
    supports_dynamic_watched_files: Arc<RwLock<bool>>,
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
        }
    }
}

#[cfg(feature = "runtime-tokio")]
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
