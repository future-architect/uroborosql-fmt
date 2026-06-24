use tower_lsp_server::lsp_types::{
    Diagnostic, DiagnosticSeverity, MessageType, NumberOrString, Position, Range, Uri,
};
use uroborosql_lint::{
    Diagnostic as SqlDiagnostic, LINT_SOURCE, LintError, Severity as SqlSeverity,
};

use crate::Backend;
use crate::document::{rope_byte_to_position, rope_char_index_to_position};
use crate::paths::{file_uri_to_path, has_parent_dir_component};

impl Backend {
    pub(crate) async fn lint_and_publish(&self, uri: &Uri, text: &str, version: Option<i32>) {
        let Some(path) = file_uri_to_path(uri) else {
            self.client
                .log_message(
                    MessageType::INFO,
                    "file URI is not a file URI; skipping lint",
                )
                .await;
            return;
        };
        // A `..` would make containment ambiguous; conformant clients never send
        // one, so surface it loudly rather than guessing the owning workspace.
        if has_parent_dir_component(&path) {
            self.client
                .log_message(
                    MessageType::WARNING,
                    format!(
                        "document path contains '..'; skipping lint: {}",
                        path.display()
                    ),
                )
                .await;
            self.client
                .publish_diagnostics(uri.clone(), vec![], version)
                .await;
            return;
        }

        // Resolve against the workspace that actually owns this document so a
        // sibling folder that merely appears first is never used by accident.
        let Some(workspace) = self.workspace_root_for_uri(uri) else {
            // The document is outside every workspace root. Publish empty rather
            // than guessing a config from an unrelated workspace.
            self.client
                .publish_diagnostics(uri.clone(), vec![], version)
                .await;
            return;
        };

        let config_store = self
            .lint_config_stores
            .read()
            .unwrap()
            .get(&workspace.path)
            .cloned()
            .flatten();
        let Some(config_store) = config_store else {
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

    let range = match (rope, span) {
        (Some(rope), Some(span)) => {
            let start = rope_byte_to_position(rope, span.start_byte);
            let end = rope_byte_to_position(rope, span.end_byte);
            Range { start, end }
        }
        // Unknown position: point at the end of the file (zero-width range).
        (Some(rope), None) => {
            let eof = rope_char_index_to_position(rope, rope.len_chars());
            Range {
                start: eof,
                end: eof,
            }
        }
        (None, _) => Range::default(),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some(LINT_SOURCE.into()),
        message: format!("Failed to parse SQL: {message}"),
        ..Diagnostic::default()
    }
}
