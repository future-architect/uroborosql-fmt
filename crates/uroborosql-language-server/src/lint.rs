use std::path::PathBuf;

use tower_lsp_server::lsp_types::{
    Diagnostic, DiagnosticSeverity, MessageType, NumberOrString, Position, Range, Uri,
};
use uroborosql_lint::{
    DEFAULT_CONFIG_FILENAME, Diagnostic as SqlDiagnostic, LINT_SOURCE, LintError,
    Severity as SqlSeverity,
};

use crate::Backend;
use crate::configuration::resolve_config_path;
use crate::document::{rope_byte_to_position, rope_char_index_to_position};
use crate::paths::file_uri_to_path;

impl Backend {
    pub(crate) fn resolve_lint_config_path(&self) -> Option<PathBuf> {
        let raw_path = self
            .client_config
            .read()
            .unwrap()
            .lint_configuration_file_path
            .clone();
        let root_dir = self.root_dir();
        resolve_config_path(root_dir.as_deref(), raw_path, DEFAULT_CONFIG_FILENAME)
    }

    pub(crate) async fn lint_and_publish(&self, uri: &Uri, text: &str, version: Option<i32>) {
        if self.root_dir().is_none() {
            return;
        }

        let Some(path) = file_uri_to_path(uri) else {
            self.client
                .log_message(
                    MessageType::INFO,
                    "file URI is not a file URI; skipping lint",
                )
                .await;
            return;
        };

        let Some(config_store) = self.lint_config_store.read().unwrap().as_ref().cloned() else {
            self.client
                .log_message(
                    MessageType::INFO,
                    "lint config store is not initialized; clearing diagnostics",
                )
                .await;
            self.client
                .publish_diagnostics(uri.clone(), vec![], version)
                .await;
            return;
        };

        if config_store.is_ignored(&path) {
            self.client
                .publish_diagnostics(uri.clone(), vec![], version)
                .await;
            return;
        }

        let resolved_config = config_store.resolve(&path);
        let rope = self.document_rope(uri);
        if rope.is_none() {
            // lint_and_publish is always called with the document already tracked,
            // so a missing rope means the document store lock is poisoned.
            self.client
                .log_message(
                    MessageType::WARNING,
                    "document rope is unavailable; diagnostic positions may be imprecise",
                )
                .await;
        }
        let diagnostics = match self.linter.run(text, &resolved_config) {
            Ok(diags) => diags
                .into_iter()
                .map(|diag| to_lsp_diagnostic(diag, rope.as_ref()))
                .collect(),
            Err(err) => vec![to_parse_error(err, rope.as_ref())],
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
    }
}

fn to_lsp_diagnostic(diag: SqlDiagnostic, rope: Option<&ropey::Rope>) -> Diagnostic {
    let severity = match diag.severity {
        SqlSeverity::Error => Some(DiagnosticSeverity::ERROR),
        SqlSeverity::Warning => Some(DiagnosticSeverity::WARNING),
        SqlSeverity::Info => Some(DiagnosticSeverity::INFORMATION),
    };

    let range = if let Some(rope) = rope {
        Range {
            start: rope_byte_to_position(rope, diag.span.start.byte),
            end: rope_byte_to_position(rope, diag.span.end.byte),
        }
    } else {
        Range {
            start: Position::new(diag.span.start.line as u32, diag.span.start.column as u32),
            end: Position::new(diag.span.end.line as u32, diag.span.end.column as u32),
        }
    };

    Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diag.code.to_string())),
        source: Some(LINT_SOURCE.into()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

fn to_parse_error(err: LintError, rope: Option<&ropey::Rope>) -> Diagnostic {
    let LintError::ParseError { message, span } = err;

    let range = match rope {
        Some(rope) => match span {
            Some(span) => {
                let start = rope_byte_to_position(rope, span.start_byte);
                let end = rope_byte_to_position(rope, span.end_byte);
                Range { start, end }
            }
            // Unknown position: point at the end of the file (zero-width range).
            None => {
                let eof = rope_char_index_to_position(rope, rope.len_chars());
                Range {
                    start: eof,
                    end: eof,
                }
            }
        },
        None => Range::default(),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some(LINT_SOURCE.into()),
        message: format!("Failed to parse SQL: {message}"),
        ..Diagnostic::default()
    }
}
