use std::path::PathBuf;

use crate::Backend;
use crate::configuration::resolve_config_path;
use crate::paths::file_uri_to_path;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::{
    Diagnostic, DiagnosticSeverity, MessageType, NumberOrString, Position, Range,
};
use uroborosql_lint::{
    DEFAULT_CONFIG_FILENAME, Diagnostic as SqlDiagnostic, LintError, Severity as SqlSeverity,
};

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

        let optional_config_store = {
            let lint_config_store_guard = self.lint_config_store.read().unwrap();
            lint_config_store_guard.as_ref().cloned()
        };

        let Some(config_store) = optional_config_store else {
            self.client
                .log_message(
                    MessageType::INFO,
                    "lint config store is not initialized; skipping to lint",
                )
                .await;
            return;
        };

        let Some(path) = file_uri_to_path(uri) else {
            self.client
                .log_message(
                    MessageType::INFO,
                    "file URI is not a file URI; skipping to lint",
                )
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
        let diagnostics = match self.linter.run(text, &resolved_config) {
            Ok(diags) => diags.into_iter().map(to_lsp_diagnostic).collect(),
            Err(err) => vec![to_parse_error(err)],
        };

        self.client
            .publish_diagnostics(uri.clone(), diagnostics, version)
            .await;
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
