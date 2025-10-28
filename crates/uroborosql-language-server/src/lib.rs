use std::sync::Arc;

use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use uroborosql_lint::{Diagnostic as SqlDiagnostic, LintError, Linter, Severity as SqlSeverity};

#[derive(Clone)]
pub struct Backend {
    client: Client,
    linter: Arc<Linter>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            linter: Arc::new(Linter::new()),
        }
    }

    async fn lint_and_publish(&self, uri: &Uri, text: &str, version: Option<i32>) {
        let diagnostics = match self.linter.run(text) {
            Ok(diags) => diags.into_iter().map(to_lsp_diagnostic).collect(),
            Err(err) => vec![to_parse_error(err)],
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        let sync_options = TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::FULL),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                include_text: Some(true),
            })),
            ..TextDocumentSyncOptions::default()
        };

        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(sync_options)),
            ..ServerCapabilities::default()
        };

        Ok(InitializeResult {
            capabilities,
            server_info: Some(ServerInfo {
                name: "uroborosql-language-server".into(),
                version: None,
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "uroborosql-language-server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text_document = params.text_document;
        let uri = text_document.uri;
        let version = text_document.version;
        let text = text_document.text;

        self.lint_and_publish(&uri, &text, Some(version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if params.content_changes.is_empty() {
            return;
        }

        // FULL sync だが、現段階では保存時にのみ診断を実行する。
        let _ = params;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.lint_and_publish(&uri, &text, None).await;
        } else {
            self.client
                .log_message(
                    MessageType::WARNING,
                    "didSave received without text; skipping lint",
                )
                .await;
        }
    }

    async fn code_action(&self, _: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        Ok(None)
    }
}

fn to_lsp_diagnostic(diag: SqlDiagnostic) -> Diagnostic {
    let severity = match diag.severity {
        SqlSeverity::Error => Some(DiagnosticSeverity::ERROR),
        SqlSeverity::Warning => Some(DiagnosticSeverity::WARNING),
        SqlSeverity::Info => Some(DiagnosticSeverity::INFORMATION),
    };

    let range = Range {
        start: Position::new(diag.span.start.line as u32, diag.span.start.column as u32),
        end: Position::new(diag.span.end.line as u32, diag.span.end.column as u32),
    };

    Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diag.rule_id.to_string())),
        source: Some("uroborosql-lint".into()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

fn to_parse_error(err: LintError) -> Diagnostic {
    let message = match err {
        LintError::ParseError(reason) => format!("Failed to parse SQL: {reason}"),
    };

    Diagnostic {
        range: Range::default(),
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some("uroborosql-lint".into()),
        message,
        ..Diagnostic::default()
    }
}

pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
