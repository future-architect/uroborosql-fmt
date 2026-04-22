mod test_harness;

use std::str::FromStr;

use tower_lsp_server::UriExt;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::notification::{Notification, PublishDiagnostics};
use tower_lsp_server::lsp_types::request::{
    RegisterCapability, Request as LspRequest, UnregisterCapability, WorkspaceConfiguration,
};

use test_harness::*;

#[tokio::test]
async fn initialize_uses_root_uri_when_workspace_folders_are_missing() {
    let mut server = new_test_server();
    let root_dir = unique_temp_dir("uroborosql-lsp-root-uri");
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("root-uri.sql")).unwrap();

    server
        .send_request(build_initialize_with_root_uri(1, Some(root_uri), false))
        .await;
    assert!(server.receive_response().await.is_ok());
    server.send_request(build_initialized()).await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), WorkspaceConfiguration::METHOD);
    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), RegisterCapability::METHOD);

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);
    assert!(
        !diag_notification.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn did_change_configuration_requests_workspace_configuration_again() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///config.sql").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let _ = server.receive_notification().await;

    server.send_request(build_did_change_configuration()).await;
    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), WorkspaceConfiguration::METHOD);
    let unregister_request = server.receive_server_request().await;
    assert_eq!(unregister_request.method(), UnregisterCapability::METHOD);
    let register_request = server.receive_server_request().await;
    assert_eq!(register_request.method(), RegisterCapability::METHOD);
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);
}
