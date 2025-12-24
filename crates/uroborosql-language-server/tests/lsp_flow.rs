use std::collections::VecDeque;
use std::str::FromStr;
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::time;
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use uroborosql_language_server::Backend;

struct TestServer {
    req_stream: DuplexStream,
    res_stream: DuplexStream,
    responses: VecDeque<String>,
    notifications: VecDeque<String>,
    server_requests: VecDeque<String>,
}

impl TestServer {
    fn new<F, S>(init: F) -> Self
    where
        F: FnOnce(Client) -> S,
        S: LanguageServer,
    {
        let (req_client, req_server) = tokio::io::duplex(2048);
        let (res_server, res_client) = tokio::io::duplex(2048);

        let (service, socket) = LspService::new(init);
        tokio::spawn(async move {
            Server::new(req_server, res_server, socket)
                .serve(service)
                .await
        });

        Self {
            req_stream: req_client,
            res_stream: res_client,
            responses: VecDeque::new(),
            notifications: VecDeque::new(),
            server_requests: VecDeque::new(),
        }
    }

    fn encode(payload: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), payload).into_bytes()
    }

    fn decode(buffer: &[u8]) -> Vec<String> {
        let mut remainder = buffer;
        let mut frames = Vec::new();
        while !remainder.is_empty() {
            let sep = match remainder.windows(4).position(|w| w == b"\r\n\r\n") {
                Some(idx) => idx + 4,
                None => break,
            };
            let (header, body) = remainder.split_at(sep);
            let len = std::str::from_utf8(header)
                .unwrap()
                .strip_prefix("Content-Length: ")
                .unwrap()
                .strip_suffix("\r\n\r\n")
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let (payload, rest) = body.split_at(len);
            frames.push(String::from_utf8(payload.to_vec()).unwrap());
            remainder = rest;
        }
        frames
    }

    async fn send_request(&mut self, req: Request) {
        let payload = serde_json::to_string(&req).unwrap();
        self.req_stream
            .write_all(&Self::encode(&payload))
            .await
            .unwrap();
    }

    async fn receive_response(&mut self) -> Response {
        loop {
            if let Some(buffer) = self.responses.pop_back() {
                return serde_json::from_str(&buffer).unwrap();
            }

            self.read_into_queues().await;
        }
    }

    async fn receive_notification(&mut self) -> Request {
        loop {
            if let Some(buffer) = self.notifications.pop_back() {
                return serde_json::from_str(&buffer).unwrap();
            }

            self.read_into_queues().await;
        }
    }

    async fn receive_server_request(&mut self) -> Request {
        loop {
            if let Some(buffer) = self.server_requests.pop_back() {
                return serde_json::from_str(&buffer).unwrap();
            }

            self.read_into_queues().await;
        }
    }

    async fn send_response(&mut self, res: Response) {
        let payload = serde_json::to_string(&res).unwrap();
        self.req_stream
            .write_all(&Self::encode(&payload))
            .await
            .unwrap();
    }

    async fn handle_server_request(&mut self, frame: String) {
        let req: Request = serde_json::from_str(&frame).unwrap();
        if let Some(id) = req.id().cloned() {
            let response = Response::from_ok(id, LSPAny::Null);
            self.send_response(response).await;
        }
        self.server_requests.push_front(frame);
    }

    async fn read_into_queues(&mut self) {
        let mut buf = vec![0u8; 4096];
        let n = self.res_stream.read(&mut buf).await.unwrap();
        for frame in Self::decode(&buf[..n]) {
            let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
            if value.get("method").is_some() {
                if value.get("id").is_some() {
                    self.handle_server_request(frame).await;
                } else {
                    if value.get("method").and_then(|method| method.as_str())
                        == Some("window/logMessage")
                    {
                        continue;
                    }
                    self.notifications.push_front(frame);
                }
            } else if value.get("id").is_some() {
                self.responses.push_front(frame);
            } else {
                self.notifications.push_front(frame);
            }
        }
    }

    async fn receive_notification_timeout(&mut self, dur: Duration) -> Option<Request> {
        match time::timeout(dur, self.receive_notification()).await {
            Ok(req) => Some(req),
            Err(_) => None,
        }
    }
}

fn build_initialize(id: i64) -> Request {
    let root_uri = Uri::from_str("file:///").expect("root uri");
    let params = InitializeParams {
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: root_uri,
            name: "uroborosql-language-server-tests".into(),
        }]),
        ..InitializeParams::default()
    };
    Request::build("initialize")
        .params(json!(params))
        .id(id)
        .finish()
}

fn build_initialized() -> Request {
    Request::build("initialized")
        .params(json!(InitializedParams {}))
        .finish()
}

fn build_did_open(uri: &Uri, text: &str, version: i32) -> Request {
    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "sql".into(),
            version,
            text: text.into(),
        },
    };
    Request::build("textDocument/didOpen")
        .params(json!(params))
        .finish()
}

fn build_did_change(uri: &Uri, version: i32, text: &str) -> Request {
    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.into(),
        }],
    };
    Request::build("textDocument/didChange")
        .params(json!(params))
        .finish()
}

fn build_did_change_range(uri: &Uri, version: i32, range: Range, text: &str) -> Request {
    let params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: Some(range),
            range_length: None,
            text: text.into(),
        }],
    };
    Request::build("textDocument/didChange")
        .params(json!(params))
        .finish()
}

fn build_did_save(uri: &Uri, text: &str) -> Request {
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        text: Some(text.into()),
    };
    Request::build("textDocument/didSave")
        .params(json!(params))
        .finish()
}

fn build_did_save_without_text(uri: &Uri) -> Request {
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        text: None,
    };
    Request::build("textDocument/didSave")
        .params(json!(params))
        .finish()
}

fn build_formatting(uri: &Uri, id: i64) -> Request {
    let params = DocumentFormattingParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        options: FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..FormattingOptions::default()
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    Request::build("textDocument/formatting")
        .params(json!(params))
        .id(id)
        .finish()
}

fn build_range_formatting(uri: &Uri, range: Range, id: i64) -> Request {
    let params = DocumentRangeFormattingParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        range,
        options: FormattingOptions {
            tab_size: 2,
            insert_spaces: true,
            ..FormattingOptions::default()
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    Request::build("textDocument/rangeFormatting")
        .params(json!(params))
        .id(id)
        .finish()
}

fn utf16_col_from_char_idx(line: &str, char_idx: usize) -> usize {
    line.chars().take(char_idx).map(|c| c.len_utf16()).sum()
}

#[tokio::test]
async fn diagnostics_publish_on_open_and_save() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///test.sql").unwrap();

    // initialize handshake
    server.send_request(build_initialize(1)).await;
    let init_res = server.receive_response().await;
    assert!(init_res.is_ok());

    server.send_request(build_initialized()).await;
    let _ = server.receive_server_request().await;

    // didOpen triggers lint (initial diagnostics)
    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(
        diag_notification.method(),
        "textDocument/publishDiagnostics"
    );
    let diagnostics = diag_notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        !diagnostics.is_empty(),
        "expected diagnostics on didOpen, got none"
    );

    // didChange should not emit diagnostics
    server
        .send_request(build_did_change(&uri, 2, "SELECT DISTINCT id FROM users;"))
        .await;
    let change_notification = server
        .receive_notification_timeout(Duration::from_millis(100))
        .await;
    assert!(
        change_notification.is_none(),
        "didChange should not publish diagnostics"
    );

    // didSave should emit diagnostics again
    server
        .send_request(build_did_save(&uri, "SELECT DISTINCT id FROM users;"))
        .await;
    let save_notification = server.receive_notification().await;
    assert_eq!(
        save_notification.method(),
        "textDocument/publishDiagnostics"
    );
}

#[tokio::test]
async fn document_formatting_returns_edit() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///fmt.sql").unwrap();

    server.send_request(build_initialize(1)).await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;
    let _ = server.receive_server_request().await;

    let original = "select A from B";
    server.send_request(build_did_open(&uri, original, 1)).await;
    let _ = server.receive_notification().await;

    server.send_request(build_formatting(&uri, 2)).await;
    let response = server.receive_response().await;
    assert!(response.is_ok());
    let value = serde_json::to_value(&response).unwrap();
    let edits = value["result"]
        .as_array()
        .expect("formatting result should be array");
    assert_eq!(edits.len(), 1);
    let new_text = edits[0]["newText"].as_str().unwrap();
    assert_ne!(
        new_text, original,
        "formatted text should differ from original"
    );
}

#[tokio::test]
async fn range_formatting_returns_edit() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///range.sql").unwrap();

    server.send_request(build_initialize(1)).await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;
    let _ = server.receive_server_request().await;

    let original = "select a from b;";
    server.send_request(build_did_open(&uri, original, 1)).await;
    let _ = server.receive_notification().await;

    let range = Range {
        start: Position::new(0, 0),
        end: Position::new(0, original.len() as u32),
    };
    server
        .send_request(build_range_formatting(&uri, range, 2))
        .await;
    let response = server.receive_response().await;
    assert!(response.is_ok());
    let value = serde_json::to_value(&response).unwrap();
    let edits = value["result"]
        .as_array()
        .expect("range formatting should return edits");
    assert_eq!(edits.len(), 1);
    assert_ne!(
        edits[0]["newText"].as_str().unwrap(),
        original,
        "range formatting should rewrite selection"
    );
}

#[tokio::test]
#[ignore = "repro: UTF-16 range change corrupts text until conversion is fixed"]
async fn utf16_range_change_should_not_corrupt_text() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///utf16.sql").unwrap();

    server.send_request(build_initialize(1)).await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;
    let _ = server.receive_server_request().await;

    let original = "select '😀' as v;";
    server.send_request(build_did_open(&uri, original, 1)).await;
    let _ = server.receive_notification().await;

    let emoji_byte = original.find('😀').expect("emoji present");
    let emoji_char_idx = original[..emoji_byte].chars().count();
    let start_col = utf16_col_from_char_idx(original, emoji_char_idx);
    let end_col = start_col + '😀'.len_utf16();
    let range = Range {
        start: Position::new(0, start_col as u32),
        end: Position::new(0, end_col as u32),
    };
    server
        .send_request(build_did_change_range(&uri, 2, range, "a"))
        .await;

    server.send_request(build_did_save_without_text(&uri)).await;
    let save_notification = server.receive_notification().await;
    assert_eq!(
        save_notification.method(),
        "textDocument/publishDiagnostics"
    );

    let diagnostics = save_notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .clone();
    let has_parse_error = diagnostics.iter().any(|diag| {
        diag.get("message")
            .and_then(|message| message.as_str())
            .map(|message| message.starts_with("Failed to parse SQL"))
            .unwrap_or(false)
    });

    assert!(
        !has_parse_error,
        "UTF-16 range change should not corrupt document text"
    );
}
