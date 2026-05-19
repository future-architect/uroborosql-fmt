mod test_harness;

use std::str::FromStr;
use std::time::Duration;

use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::notification::{Notification, PublishDiagnostics};

use test_harness::*;

#[tokio::test]
async fn diagnostics_publish_on_open_and_save() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///test.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);
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
    assert_eq!(save_notification.method(), PublishDiagnostics::METHOD);
}

#[tokio::test]
async fn did_close_clears_diagnostics() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///close.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let open_notification = server.receive_notification().await;
    assert_eq!(open_notification.method(), PublishDiagnostics::METHOD);

    server.send_request(build_did_close(&uri)).await;
    let close_notification = server.receive_notification().await;
    assert_eq!(close_notification.method(), PublishDiagnostics::METHOD);
    assert_eq!(
        close_notification.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn did_change_watched_files_relints_open_documents() {
    let mut server = new_test_server();
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
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);
}

#[tokio::test]
async fn diagnostics_include_invalid_directive_warning() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///directive.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(
            &uri,
            "-- uroborosql-lint-disable no-dstinct\nSELECT DISTINCT id FROM users;",
            1,
        ))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);

    let diagnostics = diag_notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0]["code"].as_str(),
        Some("invalid-lint-directive")
    );
    assert_eq!(diagnostics[0]["range"]["start"]["line"].as_u64(), Some(0));
    assert_eq!(
        diagnostics[0]["range"]["start"]["character"].as_u64(),
        Some(27)
    );
    assert_eq!(diagnostics[1]["code"].as_str(), Some("no-distinct"));
}
