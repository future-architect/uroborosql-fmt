mod test_harness;

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
    write_file(&root_dir.join(".uroborosqllintrc.json"), "{}");
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
async fn did_change_workspace_folders_adds_workspace_and_lints() {
    let mut server = new_test_server();
    let project_a = unique_temp_dir("uroborosql-lsp-wsf-add-a");
    write_file(&project_a.join(".uroborosqllintrc.json"), "{}");
    let project_b = unique_temp_dir("uroborosql-lsp-wsf-add-b");
    write_file(&project_b.join(".uroborosqllintrc.json"), "{}");
    let uri_b = Uri::from_file_path(project_b.join("b.sql")).unwrap();

    initialize_server_with_workspace_folders(
        &mut server,
        vec![workspace_folder(
            Uri::from_file_path(&project_a).unwrap(),
            "a",
        )],
        None,
    )
    .await;

    // Document B is outside any known workspace yet: no diagnostics.
    server
        .send_request(build_did_open(&uri_b, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let before = server.receive_notification().await;
    assert_eq!(before.method(), PublishDiagnostics::METHOD);
    assert!(
        before.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    server
        .send_request(build_did_change_workspace_folders(
            vec![workspace_folder(
                Uri::from_file_path(&project_b).unwrap(),
                "b",
            )],
            vec![],
        ))
        .await;
    let after = server.receive_notification().await;
    assert_eq!(after.method(), PublishDiagnostics::METHOD);
    assert_eq!(
        after.params().unwrap()["uri"].as_str(),
        Some(uri_b.as_str())
    );
    assert!(
        !after.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "after adding project B, its document should be linted"
    );
}

#[tokio::test]
async fn did_change_workspace_folders_removes_workspace() {
    let mut server = new_test_server();
    let project_a = unique_temp_dir("uroborosql-lsp-wsf-remove-a");
    write_file(&project_a.join(".uroborosqllintrc.json"), "{}");
    let uri = Uri::from_file_path(project_a.join("a.sql")).unwrap();

    initialize_server_with_workspace_folders(
        &mut server,
        vec![workspace_folder(
            Uri::from_file_path(&project_a).unwrap(),
            "a",
        )],
        None,
    )
    .await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let before = server.receive_notification().await;
    assert!(
        !before.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    server
        .send_request(build_did_change_workspace_folders(
            vec![],
            vec![workspace_folder(
                Uri::from_file_path(&project_a).unwrap(),
                "a",
            )],
        ))
        .await;
    let after = server.receive_notification().await;
    assert_eq!(after.method(), PublishDiagnostics::METHOD);
    assert!(
        after.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "removing the workspace should clear diagnostics"
    );
}

#[tokio::test]
async fn did_change_configuration_requests_workspace_configuration_again() {
    let mut server = new_test_server();
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-lifecycle-config")
            .await;
    let uri = Uri::from_file_path(root_dir.join("config.sql")).unwrap();

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
