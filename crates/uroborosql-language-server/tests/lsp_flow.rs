use std::collections::VecDeque;
use std::fs;
use std::str::FromStr;
use std::time::Duration;
use std::time::SystemTime;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::time;
use tower_lsp_server::UriExt;
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
    workspace_configuration_responses: VecDeque<serde_json::Value>,
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
            workspace_configuration_responses: VecDeque::new(),
        }
    }

    fn encode(payload: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{}", payload.len(), payload).into_bytes()
    }

    fn decode(buffer: &[u8]) -> Vec<String> {
        let mut remainder = buffer;
        let mut frames = Vec::new();
        while !remainder.is_empty() {
            let sep = match remainder
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
            {
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
            let result = if req.method() == "workspace/configuration" {
                self.workspace_configuration_responses
                    .pop_front()
                    .unwrap_or(LSPAny::Null)
            } else {
                LSPAny::Null
            };
            let response = Response::from_ok(id, result);
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
        time::timeout(dur, self.receive_notification()).await.ok()
    }
}

fn build_initialize(id: i64) -> Request {
    let root_uri = Uri::from_str("file:///").expect("root uri");
    build_initialize_with_root_uri(id, Some(root_uri), true)
}

#[allow(deprecated)]
fn build_initialize_with_root_uri(
    id: i64,
    root_uri: Option<Uri>,
    include_workspace_folders: bool,
) -> Request {
    let params = InitializeParams {
        root_uri: root_uri.clone(),
        workspace_folders: include_workspace_folders.then(|| {
            vec![WorkspaceFolder {
                uri: root_uri.expect("workspace folder root uri"),
                name: "uroborosql-language-server-tests".into(),
            }]
        }),
        capabilities: ClientCapabilities {
            workspace: Some(WorkspaceClientCapabilities {
                did_change_watched_files: Some(DidChangeWatchedFilesClientCapabilities {
                    dynamic_registration: Some(true),
                    relative_pattern_support: Some(false),
                }),
                ..WorkspaceClientCapabilities::default()
            }),
            ..ClientCapabilities::default()
        },
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
    Request::build("textDocument/didOpen")
        .params(json!(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "sql".into(),
                version,
                text: text.into(),
            },
        }))
        .finish()
}

fn build_did_change(uri: &Uri, version: i32, text: &str) -> Request {
    Request::build("textDocument/didChange")
        .params(json!(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.into(),
            }],
        }))
        .finish()
}

fn build_did_change_range(uri: &Uri, version: i32, range: Range, text: &str) -> Request {
    Request::build("textDocument/didChange")
        .params(json!(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(range),
                range_length: None,
                text: text.into(),
            }],
        }))
        .finish()
}

fn build_did_save(uri: &Uri, text: &str) -> Request {
    Request::build("textDocument/didSave")
        .params(json!(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some(text.into()),
        }))
        .finish()
}

fn build_did_save_without_text(uri: &Uri) -> Request {
    Request::build("textDocument/didSave")
        .params(json!(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: None,
        }))
        .finish()
}

fn build_did_close(uri: &Uri) -> Request {
    Request::build("textDocument/didClose")
        .params(json!(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        }))
        .finish()
}

fn build_did_change_configuration() -> Request {
    Request::build("workspace/didChangeConfiguration")
        .params(json!(DidChangeConfigurationParams {
            settings: serde_json::Value::Null,
        }))
        .finish()
}

fn build_did_change_watched_files(uri: Uri) -> Request {
    Request::build("workspace/didChangeWatchedFiles")
        .params(json!(DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri,
                typ: FileChangeType::CHANGED,
            }],
        }))
        .finish()
}

fn build_formatting(uri: &Uri, id: i64) -> Request {
    Request::build("textDocument/formatting")
        .params(json!(DocumentFormattingParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            options: FormattingOptions {
                tab_size: 2,
                insert_spaces: true,
                ..FormattingOptions::default()
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        }))
        .id(id)
        .finish()
}

fn build_range_formatting(uri: &Uri, range: Range, id: i64) -> Request {
    Request::build("textDocument/rangeFormatting")
        .params(json!(DocumentRangeFormattingParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range,
            options: FormattingOptions {
                tab_size: 2,
                insert_spaces: true,
                ..FormattingOptions::default()
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        }))
        .id(id)
        .finish()
}

fn utf16_col_from_char_idx(line: &str, char_idx: usize) -> usize {
    line.chars().take(char_idx).map(|c| c.len_utf16()).sum()
}

async fn initialize_server(server: &mut TestServer) {
    server.send_request(build_initialize(1)).await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), "workspace/configuration");

    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), "client/registerCapability");
}

async fn initialize_server_with_root_uri(
    server: &mut TestServer,
    root_uri: Uri,
    workspace_config: Option<serde_json::Value>,
) {
    if let Some(workspace_config) = workspace_config {
        server
            .workspace_configuration_responses
            .push_back(workspace_config);
    }

    server
        .send_request(build_initialize_with_root_uri(1, Some(root_uri), true))
        .await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), "workspace/configuration");

    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), "client/registerCapability");
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

#[tokio::test]
async fn diagnostics_publish_on_open_and_save() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///test.sql").unwrap();

    initialize_server(&mut server).await;

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
    assert!(!diagnostics.is_empty());

    server
        .send_request(build_did_change(&uri, 2, "SELECT DISTINCT id FROM users;"))
        .await;
    let change_notification = server
        .receive_notification_timeout(Duration::from_millis(100))
        .await;
    assert!(change_notification.is_none());

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
async fn did_close_clears_diagnostics() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///close.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let open_notification = server.receive_notification().await;
    assert_eq!(
        open_notification.method(),
        "textDocument/publishDiagnostics"
    );

    server.send_request(build_did_close(&uri)).await;
    let close_notification = server.receive_notification().await;
    assert_eq!(
        close_notification.method(),
        "textDocument/publishDiagnostics"
    );
    assert_eq!(
        close_notification.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn did_change_configuration_requests_workspace_configuration_again() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///config.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let _ = server.receive_notification().await;

    server.send_request(build_did_change_configuration()).await;
    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), "workspace/configuration");
    let unregister_request = server.receive_server_request().await;
    assert_eq!(unregister_request.method(), "client/unregisterCapability");
    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), "client/registerCapability");
    let diag_notification = server.receive_notification().await;
    assert_eq!(
        diag_notification.method(),
        "textDocument/publishDiagnostics"
    );
}

#[tokio::test]
async fn document_formatting_returns_edit() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///fmt.sql").unwrap();

    initialize_server(&mut server).await;

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
    assert_ne!(edits[0]["newText"].as_str().unwrap(), original);
}

#[tokio::test]
async fn range_formatting_returns_edit() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///range.sql").unwrap();

    initialize_server(&mut server).await;

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
    assert_ne!(edits[0]["newText"].as_str().unwrap(), original);
}

#[tokio::test]
async fn utf16_range_change_should_not_corrupt_text() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///utf16.sql").unwrap();

    initialize_server(&mut server).await;

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

    assert!(!has_parse_error);
}

#[tokio::test]
async fn diagnostics_range_uses_utf16_columns() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///utf16-diagnostics.sql").unwrap();

    initialize_server(&mut server).await;

    let original = "SELECT DISTINCT '😀a';";
    server.send_request(build_did_open(&uri, original, 1)).await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(
        diag_notification.method(),
        "textDocument/publishDiagnostics"
    );

    let diagnostics = diag_notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap();
    let first = diagnostics.first().expect("diagnostic should exist");
    let start = &first["range"]["start"];
    let end = &first["range"]["end"];

    assert_eq!(start["line"].as_u64(), Some(0));
    assert_eq!(start["character"].as_u64(), Some(7));
    assert_eq!(end["line"].as_u64(), Some(0));
    assert_eq!(end["character"].as_u64(), Some(15));
}

#[tokio::test]
async fn initialize_uses_root_uri_when_workspace_folders_are_missing() {
    let mut server = TestServer::new(Backend::new);
    let root_dir = unique_temp_dir("uroborosql-lsp-root-uri");
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("root-uri.sql")).unwrap();

    server
        .send_request(build_initialize_with_root_uri(1, Some(root_uri), false))
        .await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), "workspace/configuration");
    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), "client/registerCapability");

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(
        diag_notification.method(),
        "textDocument/publishDiagnostics"
    );
    assert!(
        !diag_notification.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn did_change_watched_files_relints_open_documents() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///watched.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let _ = server.receive_notification().await;

    server
        .send_request(build_did_change_watched_files(
            Uri::from_str("file:///tmp/.uroborosqllintrc.json").unwrap(),
        ))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(
        diag_notification.method(),
        "textDocument/publishDiagnostics"
    );
}

#[tokio::test]
async fn formatting_merges_config_file_with_explicit_client_overrides() {
    let mut server = TestServer::new(Backend::new);
    let root_dir = unique_temp_dir("uroborosql-lsp-format-merge");
    write_file(
        &root_dir.join(".uroborosqlfmtrc.json"),
        r#"{
  "keyword_case": "upper",
  "identifier_case": "upper"
}"#,
    );
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("format.sql")).unwrap();

    initialize_server_with_root_uri(
        &mut server,
        root_uri,
        Some(json!([
            {
                "configurationFilePath": ".uroborosqlfmtrc.json",
                "lintConfigurationFilePath": "",
                "keywordCase": "lower"
            }
        ])),
    )
    .await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let _ = server.receive_notification().await;

    server.send_request(build_formatting(&uri, 2)).await;
    let response = server.receive_response().await;
    assert!(response.is_ok());

    let value = serde_json::to_value(&response).unwrap();
    let edits = value["result"]
        .as_array()
        .expect("formatting result should be array");
    let new_text = edits[0]["newText"]
        .as_str()
        .expect("text edit should contain newText");

    assert!(new_text.starts_with("select\n"));
    assert!(new_text.contains("distinct"));
    assert!(new_text.contains("\nfrom\n"));
    assert!(new_text.contains("ID"));
    assert!(new_text.contains("USERS"));
    assert!(!new_text.contains("SELECT"));
    assert!(!new_text.contains("DISTINCT"));
    assert!(!new_text.contains("FROM"));
}

#[tokio::test]
async fn formatting_returns_null_when_explicit_config_file_is_missing() {
    let mut server = TestServer::new(Backend::new);
    let root_dir = unique_temp_dir("uroborosql-lsp-missing-config");
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("missing-config.sql")).unwrap();

    initialize_server_with_root_uri(
        &mut server,
        root_uri,
        Some(json!([
            {
                "configurationFilePath": "missing.json",
                "keywordCase": "lower"
            }
        ])),
    )
    .await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let _ = server.receive_notification().await;

    server.send_request(build_formatting(&uri, 2)).await;
    let response = server.receive_response().await;
    assert!(response.is_ok());

    let value = serde_json::to_value(&response).unwrap();
    assert!(value["result"].is_null());
}

#[tokio::test]
async fn utf16_range_formatting_returns_utf16_safe_range() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///utf16-range.sql").unwrap();

    initialize_server(&mut server).await;

    let original = "select '😀a';";
    server.send_request(build_did_open(&uri, original, 1)).await;
    let _ = server.receive_notification().await;

    let end_col = utf16_col_from_char_idx(original, original.chars().count());
    let range = Range {
        start: Position::new(0, 0),
        end: Position::new(0, end_col as u32),
    };
    server
        .send_request(build_range_formatting(&uri, range, 2))
        .await;
    let response = server.receive_response().await;
    assert!(response.is_ok());
    let value = serde_json::to_value(&response).unwrap();
    let edit = &value["result"].as_array().unwrap()[0];
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(0));
    assert_eq!(
        edit["range"]["end"]["character"].as_u64(),
        Some(end_col as u64)
    );
}
