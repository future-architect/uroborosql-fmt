use tower_lsp_server::LanguageServer;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::lsp_types::notification::{DidChangeWatchedFiles, Notification};
use tower_lsp_server::lsp_types::*;
use uroborosql_fmt::format_sql;
use uroborosql_lint::DEFAULT_CONFIG_FILENAME;

use crate::Backend;
use crate::document::{rope_char_index_to_position, rope_position_to_char_index};

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // This server only supports a single workspace folder for now, so it uses the first one.
        // `InitializeParams::root_uri` is deprecated, but some clients still omit
        // `workspaceFolders`, so keep `rootUri` as a fallback for compatibility.
        #[allow(deprecated)]
        let root_uri = params
            .workspace_folders
            .as_ref()
            .and_then(|folders| folders.first().map(|folder| folder.uri.clone()))
            .or(params.root_uri);
        *self.root_uri.write().unwrap() = root_uri.clone();

        if root_uri.is_none() {
            self.client
                .log_message(
                    MessageType::INFO,
                    "no workspace folders or rootUri provided",
                )
                .await;
        }

        let supports_dynamic_watched_files = params
            .capabilities
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.did_change_watched_files.as_ref())
            .and_then(|capability| capability.dynamic_registration)
            .unwrap_or(false);
        *self.supports_dynamic_watched_files.write().unwrap() = supports_dynamic_watched_files;

        let sync_options = TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::INCREMENTAL),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                include_text: Some(true),
            })),
            ..TextDocumentSyncOptions::default()
        };

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(sync_options)),
                document_formatting_provider: Some(OneOf::Left(true)),
                document_range_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
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

        self.refresh_client_config().await;
        self.refresh_lint_config_store().await;
        self.sync_watched_files_registration().await;
    }

    async fn did_change_configuration(&self, _: DidChangeConfigurationParams) {
        self.refresh_client_config().await;
        self.refresh_lint_config_store().await;
        self.sync_watched_files_registration().await;
        self.relint_open_documents().await;
    }

    async fn did_change_watched_files(&self, _: DidChangeWatchedFilesParams) {
        self.refresh_lint_config_store().await;
        self.relint_open_documents().await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let text_document = params.text_document;
        let uri = text_document.uri;
        let version = text_document.version;
        let text = text_document.text;

        self.upsert_document(&uri, &text, Some(version));
        self.lint_and_publish(&uri, &text, Some(version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if params.content_changes.is_empty() {
            return;
        }

        let uri = params.text_document.uri;
        let version = params.text_document.version;
        for change in params.content_changes {
            if change.range.is_some() {
                self.apply_change(&uri, change, version);
            } else {
                self.upsert_document(&uri, &change.text, Some(version));
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.remove_document(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(text) = params.text {
            self.upsert_document(&uri, &text, None);
            self.lint_and_publish(&uri, &text, None).await;
        } else if let Some(text) = self.document_text(&uri) {
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

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(rope) = self.document_rope(&uri) else {
            return Ok(None);
        };
        let text = rope.to_string();

        let fmt_config_path = self.resolve_fmt_config_path();
        let fmt_config_path = fmt_config_path.as_ref().and_then(|path| path.to_str());
        let client_config_json = self.client_config_json_explicit_only();

        match format_sql(&text, Some(&client_config_json), fmt_config_path) {
            Ok(formatted) => {
                if formatted == text {
                    return Ok(Some(vec![]));
                }

                Ok(Some(vec![TextEdit {
                    range: Range {
                        start: Position::new(0, 0),
                        end: rope_char_index_to_position(&rope, rope.len_chars()),
                    },
                    new_text: formatted,
                }]))
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("formatting failed for {}: {err}", uri.as_str()),
                    )
                    .await;
                Ok(None)
            }
        }
    }

    async fn range_formatting(
        &self,
        params: DocumentRangeFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let Some(rope) = self.document_rope(&uri) else {
            return Ok(None);
        };

        let Some(start_char) = rope_position_to_char_index(&rope, params.range.start) else {
            return Ok(None);
        };
        let Some(end_char) = rope_position_to_char_index(&rope, params.range.end) else {
            return Ok(None);
        };
        if start_char > end_char || end_char > rope.len_chars() {
            return Ok(None);
        }

        let slice = rope.slice(start_char..end_char).to_string();
        let fmt_config_path = self.resolve_fmt_config_path();
        let fmt_config_path = fmt_config_path.as_ref().and_then(|path| path.to_str());
        let client_config_json = self.client_config_json_explicit_only();

        match format_sql(&slice, Some(&client_config_json), fmt_config_path) {
            Ok(formatted) => Ok(Some(vec![TextEdit {
                range: Range {
                    start: rope_char_index_to_position(&rope, start_char),
                    end: rope_char_index_to_position(&rope, end_char),
                },
                new_text: formatted,
            }])),
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("range formatting failed for {}: {err}", uri.as_str()),
                    )
                    .await;
                Ok(None)
            }
        }
    }
}

impl Backend {
    async fn relint_open_documents(&self) {
        for (uri, text, version) in self.open_documents() {
            self.lint_and_publish(&uri, &text, Some(version)).await;
        }
    }

    fn watched_file_patterns(&self) -> Vec<String> {
        vec![format!("**/{DEFAULT_CONFIG_FILENAME}")]
    }

    async fn sync_watched_files_registration(&self) {
        if !*self.supports_dynamic_watched_files.read().unwrap() {
            return;
        }

        if *self.has_watched_files_registration.read().unwrap() {
            let unregisterations = vec![Unregistration {
                id: "uroborosql-fmt-watcher".to_string(),
                method: DidChangeWatchedFiles::METHOD.to_string(),
            }];
            if let Err(err) = self.client.unregister_capability(unregisterations).await {
                self.client
                    .log_message(
                        MessageType::WARNING,
                        format!("client/unregisterCapability failed: {err}"),
                    )
                    .await;
            }
        }

        let register_options = DidChangeWatchedFilesRegistrationOptions {
            watchers: self
                .watched_file_patterns()
                .into_iter()
                .map(|pattern| FileSystemWatcher {
                    glob_pattern: GlobPattern::String(pattern),
                    kind: None,
                })
                .collect(),
        };

        let registrations = vec![Registration {
            id: "uroborosql-fmt-watcher".to_string(),
            method: DidChangeWatchedFiles::METHOD.to_string(),
            register_options: Some(serde_json::to_value(register_options).unwrap()),
        }];

        match self.client.register_capability(registrations).await {
            Ok(()) => {
                *self.has_watched_files_registration.write().unwrap() = true;
            }
            Err(err) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("client/registerCapability failed: {err}"),
                    )
                    .await;
            }
        }
    }
}
