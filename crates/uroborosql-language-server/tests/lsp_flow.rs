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
        if let Some(buffer) = self.responses.pop_back() {
            return serde_json::from_str(&buffer).unwrap();
        }

        let mut buf = vec![0u8; 4096];
        let n = self.res_stream.read(&mut buf).await.unwrap();
        for frame in Self::decode(&buf[..n]) {
            self.responses.push_front(frame);
        }
        let msg = self.responses.pop_back().unwrap();
        serde_json::from_str(&msg).unwrap()
    }

    async fn receive_notification(&mut self) -> Request {
        if let Some(buffer) = self.responses.pop_back() {
            return serde_json::from_str(&buffer).unwrap();
        }

        let mut buf = vec![0u8; 4096];
        let n = self.res_stream.read(&mut buf).await.unwrap();
        for frame in Self::decode(&buf[..n]) {
            self.responses.push_front(frame);
        }
        let msg = self.responses.pop_back().unwrap();
        serde_json::from_str(&msg).unwrap()
    }

    async fn receive_notification_timeout(&mut self, dur: Duration) -> Option<Request> {
        match time::timeout(dur, self.receive_notification()).await {
            Ok(req) => Some(req),
            Err(_) => None,
        }
    }
}

fn build_initialize(id: i64) -> Request {
    Request::build("initialize")
        .params(json!(InitializeParams::default()))
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

fn build_did_save(uri: &Uri, text: &str) -> Request {
    let params = DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        text: Some(text.into()),
    };
    Request::build("textDocument/didSave")
        .params(json!(params))
        .finish()
}

#[tokio::test]
async fn diagnostics_publish_on_save_only() {
    let mut server = TestServer::new(Backend::new);
    let uri = Uri::from_str("file:///test.sql").unwrap();

    // initialize handshake
    server.send_request(build_initialize(1)).await;
    let init_res = server.receive_response().await;
    assert!(init_res.is_ok());

    server.send_request(build_initialized()).await;
    let init_notification = server.receive_notification().await;
    assert_eq!(init_notification.method(), "window/logMessage");

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
