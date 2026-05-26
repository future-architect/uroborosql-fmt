#![allow(dead_code)]

use std::collections::VecDeque;
use std::fs;
use std::str::FromStr;
use std::time::SystemTime;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tower_lsp_server::jsonrpc::{Request, Response};
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, DidSaveTextDocument, Initialized, LogMessage, Notification,
};
use tower_lsp_server::lsp_types::request::{
    Formatting, Initialize, RangeFormatting, RegisterCapability, Request as LspRequest,
    WorkspaceConfiguration,
};
use tower_lsp_server::lsp_types::*;
use tower_lsp_server::{Client, LanguageServer, LspService, Server};
use uroborosql_language_server::create_service;

pub(crate) struct TestServer {
    req_stream: DuplexStream,
    res_stream: DuplexStream,
    responses: VecDeque<String>,
    notifications: VecDeque<String>,
    server_requests: VecDeque<String>,
    workspace_configuration_responses: VecDeque<serde_json::Value>,
}

impl TestServer {
    pub(crate) fn new<F, S>(init: F) -> Self
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

    pub(crate) async fn send_request(&mut self, req: Request) {
        let payload = serde_json::to_string(&req).unwrap();
        self.req_stream
            .write_all(&Self::encode(&payload))
            .await
            .unwrap();
    }

    pub(crate) async fn receive_response(&mut self) -> Response {
        loop {
            if let Some(buffer) = self.responses.pop_back() {
                return serde_json::from_str(&buffer).unwrap();
            }
            self.read_into_queues().await;
        }
    }

    pub(crate) async fn receive_notification(&mut self) -> Request {
        loop {
            if let Some(buffer) = self.notifications.pop_back() {
                return serde_json::from_str(&buffer).unwrap();
            }
            self.read_into_queues().await;
        }
    }

    pub(crate) async fn receive_server_request(&mut self) -> Request {
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
            let result = if req.method() == WorkspaceConfiguration::METHOD {
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
                        == Some(LogMessage::METHOD)
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

    pub(crate) async fn receive_notification_timeout(
        &mut self,
        dur: std::time::Duration,
    ) -> Option<Request> {
        tokio::time::timeout(dur, self.receive_notification())
            .await
            .ok()
    }

    pub(crate) fn push_workspace_configuration_response(&mut self, value: serde_json::Value) {
        self.workspace_configuration_responses.push_back(value);
    }
}

pub(crate) fn new_test_server() -> TestServer {
    let (req_client, req_server) = tokio::io::duplex(2048);
    let (res_server, res_client) = tokio::io::duplex(2048);

    let (service, socket) = create_service();
    tokio::spawn(async move {
        Server::new(req_server, res_server, socket)
            .serve(service)
            .await
    });

    TestServer {
        req_stream: req_client,
        res_stream: res_client,
        responses: VecDeque::new(),
        notifications: VecDeque::new(),
        server_requests: VecDeque::new(),
        workspace_configuration_responses: VecDeque::new(),
    }
}

pub(crate) fn build_initialize(id: i64) -> Request {
    let root_uri = Uri::from_str("file:///").expect("root uri");
    build_initialize_with_root_uri(id, Some(root_uri), true)
}

#[allow(deprecated)]
pub(crate) fn build_initialize_with_root_uri(
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
    Request::build(Initialize::METHOD)
        .params(json!(params))
        .id(id)
        .finish()
}

pub(crate) fn build_initialized() -> Request {
    Request::build(Initialized::METHOD)
        .params(json!(InitializedParams {}))
        .finish()
}

pub(crate) fn build_did_open(uri: &Uri, text: &str, version: i32) -> Request {
    Request::build(DidOpenTextDocument::METHOD)
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

pub(crate) fn build_did_change(uri: &Uri, version: i32, text: &str) -> Request {
    Request::build(DidChangeTextDocument::METHOD)
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

pub(crate) fn build_did_change_range(uri: &Uri, version: i32, range: Range, text: &str) -> Request {
    Request::build(DidChangeTextDocument::METHOD)
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

pub(crate) fn build_did_save(uri: &Uri, text: &str) -> Request {
    Request::build(DidSaveTextDocument::METHOD)
        .params(json!(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: Some(text.into()),
        }))
        .finish()
}

pub(crate) fn build_did_save_without_text(uri: &Uri) -> Request {
    Request::build(DidSaveTextDocument::METHOD)
        .params(json!(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text: None,
        }))
        .finish()
}

pub(crate) fn build_did_close(uri: &Uri) -> Request {
    Request::build(DidCloseTextDocument::METHOD)
        .params(json!(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        }))
        .finish()
}

pub(crate) fn build_did_change_configuration() -> Request {
    Request::build(DidChangeConfiguration::METHOD)
        .params(json!(DidChangeConfigurationParams {
            settings: serde_json::Value::Null,
        }))
        .finish()
}

pub(crate) fn build_did_change_watched_files(uri: Uri) -> Request {
    Request::build(DidChangeWatchedFiles::METHOD)
        .params(json!(DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri,
                typ: FileChangeType::CHANGED,
            }],
        }))
        .finish()
}

pub(crate) fn build_formatting(uri: &Uri, id: i64) -> Request {
    Request::build(Formatting::METHOD)
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

pub(crate) fn build_range_formatting(uri: &Uri, range: Range, id: i64) -> Request {
    Request::build(RangeFormatting::METHOD)
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

pub(crate) fn build_format_selections_as_sql(
    uri: &Uri,
    version: i32,
    selections: serde_json::Value,
    id: i64,
) -> Request {
    Request::build("uroborosql/formatSelectionsAsSql")
        .params(json!({
            "hostDocumentUri": uri,
            "hostDocumentVersion": version,
            "selections": selections,
        }))
        .id(id)
        .finish()
}

pub(crate) fn utf16_col_from_char_idx(line: &str, char_idx: usize) -> usize {
    line.chars().take(char_idx).map(|c| c.len_utf16()).sum()
}

pub(crate) async fn initialize_server(server: &mut TestServer) {
    server.send_request(build_initialize(1)).await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), WorkspaceConfiguration::METHOD);

    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), RegisterCapability::METHOD);
}

pub(crate) async fn initialize_server_with_root_uri(
    server: &mut TestServer,
    root_uri: Uri,
    workspace_config: Option<serde_json::Value>,
) {
    if let Some(workspace_config) = workspace_config {
        server.push_workspace_configuration_response(workspace_config);
    }

    server
        .send_request(build_initialize_with_root_uri(1, Some(root_uri), true))
        .await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), WorkspaceConfiguration::METHOD);

    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), RegisterCapability::METHOD);
}

pub(crate) fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn write_file(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}
