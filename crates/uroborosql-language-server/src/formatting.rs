use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tower_lsp_server::jsonrpc::{Error, ErrorCode, Result};
use tower_lsp_server::lsp_types::{Range, Uri};
use uroborosql_fmt::format_sql;

use crate::configuration::{ClientConfig, resolve_config_path};
use crate::{Backend, DEFAULT_FMT_CONFIG_PATH};

pub(crate) const FORMAT_SELECTIONS_AS_SQL_METHOD: &str = "uroborosql/formatSelectionsAsSql";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormatSelectionAsSql {
    pub(crate) range: Range,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormatSelectionsAsSqlParams {
    pub(crate) host_document_uri: Uri,
    pub(crate) host_document_version: i32,
    pub(crate) selections: Vec<FormatSelectionAsSql>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormatSelectionAsSqlEdit {
    pub(crate) range: Range,
    pub(crate) new_text: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormatSelectionsAsSqlResult {
    pub(crate) host_document_version: i32,
    pub(crate) edits: Vec<FormatSelectionAsSqlEdit>,
}

impl Backend {
    pub(crate) fn resolve_fmt_config_path_for_uri(
        &self,
        uri: &Uri,
        config: &ClientConfig,
    ) -> Option<PathBuf> {
        let raw_path = config.configuration_file_path.clone();
        let root_dir = self.workspace_dir_for_uri(uri);
        resolve_config_path(root_dir.as_deref(), raw_path, DEFAULT_FMT_CONFIG_PATH)
    }

    pub(crate) fn client_config_json_explicit_only_for(&self, config: &ClientConfig) -> String {
        serde_json::to_string(&config.formatter).expect("PartialConfig must be serializable")
    }

    pub(crate) async fn formatter_settings_for_uri(
        &self,
        scope_uri: &Uri,
    ) -> (Option<PathBuf>, String) {
        let config = self
            .fetch_client_config(Some(scope_uri.clone()))
            .await
            .unwrap_or_else(|| self.cached_workspace_config_for_uri(scope_uri));
        (
            self.resolve_fmt_config_path_for_uri(scope_uri, &config),
            self.client_config_json_explicit_only_for(&config),
        )
    }

    pub(crate) async fn format_sql_with_uri(
        &self,
        text: &str,
        scope_uri: &Uri,
        operation_name: &str,
    ) -> std::result::Result<String, Error> {
        let (fmt_config_path, client_config_json) =
            self.formatter_settings_for_uri(scope_uri).await;
        let fmt_config_path = fmt_config_path.as_ref().and_then(|path| path.to_str());

        format_sql(text, Some(&client_config_json), fmt_config_path).map_err(|err| Error {
            code: ErrorCode::InternalError,
            message: format!("{operation_name} failed for {}: {err}", scope_uri.as_str()).into(),
            data: None,
        })
    }

    pub(crate) async fn format_selections_as_sql(
        &self,
        params: FormatSelectionsAsSqlParams,
    ) -> Result<FormatSelectionsAsSqlResult> {
        if params.selections.is_empty() {
            return Err(Error::invalid_params("selections must not be empty"));
        }

        let (fmt_config_path, client_config_json) = self
            .formatter_settings_for_uri(&params.host_document_uri)
            .await;
        let fmt_config_path = fmt_config_path.as_ref().and_then(|path| path.to_str());

        let mut edits = Vec::with_capacity(params.selections.len());
        for selection in params.selections {
            let new_text = format_sql(&selection.text, Some(&client_config_json), fmt_config_path)
                .map_err(|err| Error {
                    code: ErrorCode::InternalError,
                    message: format!(
                        "formatSelectionsAsSql failed for {}: {err}",
                        params.host_document_uri.as_str()
                    )
                    .into(),
                    data: None,
                })?;
            edits.push(FormatSelectionAsSqlEdit {
                range: selection.range,
                new_text,
            });
        }

        Ok(FormatSelectionsAsSqlResult {
            host_document_version: params.host_document_version,
            edits,
        })
    }
}
