use crate::text::rope_char_to_position;
use crate::text::rope_position_to_char;
use crate::Backend;
use tower_lsp_server::jsonrpc::Result;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::LanguageServer;
use uroborosql_fmt::format_sql;

impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        let sync_options = TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::INCREMENTAL),
            save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                include_text: Some(true),
            })),
            ..TextDocumentSyncOptions::default()
        };

        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(sync_options)),
            document_formatting_provider: Some(OneOf::Left(true)),
            document_range_formatting_provider: Some(OneOf::Left(true)),
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

        self.upsert_document(&uri, &text, Some(version));

        if let Err(err) = self.lint_and_publish(&uri, &text, Some(version)).await {
            self.client.log_message(MessageType::WARNING, err).await;
        }
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
            if let Err(err) = self.lint_and_publish(&uri, &text, None).await {
                self.client.log_message(MessageType::WARNING, err).await;
            }
        } else if let Some(text) = self.document_text(&uri) {
            if let Err(err) = self.lint_and_publish(&uri, &text, None).await {
                self.client.log_message(MessageType::WARNING, err).await;
            }
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
        let rope = match self.document_rope(&uri) {
            Some(rope) => rope,
            None => return Ok(None),
        };
        let text = rope.to_string();

        match format_sql(&text, None, None) {
            Ok(formatted) => {
                if formatted == text {
                    return Ok(Some(vec![]));
                }

                let end = rope_char_to_position(&rope, rope.len_chars());
                // replace the entire document
                let edit = TextEdit {
                    range: Range {
                        start: Position::new(0, 0),
                        end,
                    },
                    new_text: formatted,
                };

                Ok(Some(vec![edit]))
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
        let rope = match self.document_rope(&uri) {
            Some(rope) => rope,
            None => return Ok(None),
        };

        let start_char = match rope_position_to_char(&rope, params.range.start) {
            Some(pos) => pos,
            None => return Ok(None),
        };
        let end_char = match rope_position_to_char(&rope, params.range.end) {
            Some(pos) => pos,
            None => return Ok(None),
        };
        if start_char > end_char || end_char > rope.len_chars() {
            return Ok(None);
        }

        let slice = rope.slice(start_char..end_char).to_string();
        // ignore settings for now
        match format_sql(&slice, None, None) {
            Ok(formatted) => {
                let edit = TextEdit {
                    range: Range {
                        start: rope_char_to_position(&rope, start_char),
                        end: rope_char_to_position(&rope, end_char),
                    },
                    new_text: formatted,
                };
                Ok(Some(vec![edit]))
            }
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
