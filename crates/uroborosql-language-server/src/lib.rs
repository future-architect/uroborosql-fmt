mod server;
mod text;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use ropey::Rope;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::*;
// Assuming UriExt is available in tower_lsp_server or needs to be imported if suggested?
// Actually the suggestion says `use tower_lsp_server::UriExt`. Let's assume it exists or check imports.
// But wait, standard Url from `url` crate has `to_file_path`.
// If `Uri` is `lsp_types::Url`, it should have it.
// use tower_lsp_server::ClientSocket;
#[cfg(feature = "runtime-tokio")]
use tower_lsp_server::Server;
use tower_lsp_server::{Client, LspService, UriExt};
use uroborosql_lint::{Diagnostic as SqlDiagnostic, LintError, Linter, Severity as SqlSeverity};

use crate::text::rope_range_to_char_range;

#[derive(Clone)]
pub struct Backend {
    client: Client,
    linter: Arc<Linter>,
    documents: Arc<RwLock<HashMap<Uri, DocumentState>>>,
}

#[derive(Clone)]
struct DocumentState {
    rope: Rope,
    version: i32,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            linter: Arc::new(Linter::new()),
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn lint_and_publish(
        &self,
        uri: &Uri,
        text: &str,
        version: Option<i32>,
    ) -> Result<(), String> {
        let path = uri
            .to_file_path()
            .ok_or_else(|| format!("Invalid file URI: {}", uri.as_str()))?;
        let diagnostics = match self.linter.run(&path, text) {
            Ok(diags) => diags.into_iter().map(to_lsp_diagnostic).collect(),
            Err(err) => vec![to_parse_error(err)],
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
        Ok(())
    }

    fn upsert_document(&self, uri: &Uri, text: &str, version: Option<i32>) {
        let resolved_version = version.or_else(|| {
            self.documents
                .read()
                .ok()
                .and_then(|docs| docs.get(uri).map(|doc| doc.version))
        });
        let version = resolved_version.unwrap_or_default();

        if let Ok(mut docs) = self.documents.write() {
            docs.insert(
                uri.clone(),
                DocumentState {
                    rope: Rope::from_str(text),
                    version,
                },
            );
        }
    }

    fn apply_change(&self, uri: &Uri, change: TextDocumentContentChangeEvent, version: i32) {
        if let Ok(mut docs) = self.documents.write() {
            if let Some(doc) = docs.get_mut(uri) {
                if version < doc.version {
                    return;
                }
                doc.version = version;
                if let Some(range) = change.range {
                    if let Some((start, end)) = rope_range_to_char_range(&doc.rope, &range) {
                        doc.rope.remove(start..end);
                        doc.rope.insert(start, &change.text);
                    }
                } else {
                    doc.rope = Rope::from_str(&change.text);
                }
            }
        }
    }

    fn remove_document(&self, uri: &Uri) {
        if let Ok(mut docs) = self.documents.write() {
            docs.remove(uri);
        }
    }

    fn document_rope(&self, uri: &Uri) -> Option<Rope> {
        self.documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).map(|doc| doc.rope.clone()))
    }

    fn document_text(&self, uri: &Uri) -> Option<String> {
        self.document_rope(uri).map(|rope| rope.to_string())
    }
}

#[cfg(feature = "runtime-tokio")]
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
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
