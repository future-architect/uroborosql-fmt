mod test_harness;

use std::str::FromStr;
use std::time::Duration;

use serde_json::json;
use tower_lsp_server::UriExt;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::notification::{Notification, PublishDiagnostics};

use test_harness::*;

#[tokio::test]
async fn diagnostics_publish_on_open_and_save() {
    let mut server = new_test_server();
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-diag-open").await;
    let uri = Uri::from_file_path(root_dir.join("test.sql")).unwrap();

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
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-diag-close").await;
    let uri = Uri::from_file_path(root_dir.join("close.sql")).unwrap();

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
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-diag-watched")
            .await;
    let uri = Uri::from_file_path(root_dir.join("watched.sql")).unwrap();

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
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-diag-directive")
            .await;
    let uri = Uri::from_file_path(root_dir.join("directive.sql")).unwrap();

    server
        .send_request(build_did_open(
            &uri,
            "-- uroborosql-lint-disable definitely-not-a-rule\nSELECT DISTINCT id FROM users;",
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

#[tokio::test]
async fn parse_error_diagnostic_points_at_error_location() {
    let mut server = new_test_server();
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-diag-parse-error")
            .await;
    let uri = Uri::from_file_path(root_dir.join("invalid.sql")).unwrap();

    // Invalid SQL: the WHERE on line 2 has no condition, so it errors at end of input.
    server
        .send_request(build_did_open(&uri, "SELECT id\nFROM users WHERE", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);

    let diagnostics = diag_notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(diagnostics.len(), 1);

    let range = &diagnostics[0]["range"];
    let start_line = range["start"]["line"].as_u64();
    let start_character = range["start"]["character"].as_u64();
    assert_eq!(start_line, Some(1));
    assert_eq!(start_character, Some(16));
    assert!(
        !(start_line == Some(0) && start_character == Some(0)),
        "parse error must not collapse to 0,0"
    );
}

#[tokio::test]
async fn lint_picks_each_documents_own_workspace_config() {
    let mut server = new_test_server();
    let project_a = unique_temp_dir("uroborosql-lsp-multi-a");
    let project_b = unique_temp_dir("uroborosql-lsp-multi-b");
    write_file(&project_a.join(".uroborosqllintrc.json"), "{}");
    write_file(
        &project_b.join(".uroborosqllintrc.json"),
        r#"{ "rules": { "no-distinct": "off" } }"#,
    );

    let uri_a = Uri::from_file_path(project_a.join("a.sql")).unwrap();
    let uri_b = Uri::from_file_path(project_b.join("b.sql")).unwrap();
    initialize_server_with_workspace_folders(
        &mut server,
        vec![
            workspace_folder(Uri::from_file_path(&project_a).unwrap(), "a"),
            workspace_folder(Uri::from_file_path(&project_b).unwrap(), "b"),
        ],
        None,
    )
    .await;

    server
        .send_request(build_did_open(&uri_a, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_a = server.receive_notification().await;
    assert_eq!(
        diag_a.params().unwrap()["uri"].as_str(),
        Some(uri_a.as_str())
    );
    assert!(
        !diag_a.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "project A should report no-distinct"
    );

    server
        .send_request(build_did_open(&uri_b, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_b = server.receive_notification().await;
    assert_eq!(
        diag_b.params().unwrap()["uri"].as_str(),
        Some(uri_b.as_str())
    );
    assert!(
        diag_b.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "project B disables no-distinct, so it should report nothing"
    );
}

#[tokio::test]
async fn lint_resolves_per_root_lint_config_path_override() {
    let mut server = new_test_server();
    let project_a = unique_temp_dir("uroborosql-lsp-per-root-a");
    let project_b = unique_temp_dir("uroborosql-lsp-per-root-b");

    // Project A: the default file would report, but its explicit override turns
    // no-distinct off, so honoring the override means A stays silent.
    write_file(&project_a.join(".uroborosqllintrc.json"), "{}");
    write_file(
        &project_a.join("a-config.json"),
        r#"{ "rules": { "no-distinct": "off" } }"#,
    );
    // Project B keeps no-distinct on, but only under its own override filename.
    // If B were resolved with A's path ("a-config.json"), B would find nothing.
    write_file(&project_b.join("b-config.json"), "{}");

    let uri_a = Uri::from_file_path(project_a.join("a.sql")).unwrap();
    let uri_b = Uri::from_file_path(project_b.join("b.sql")).unwrap();
    initialize_server_with_workspace_folder_configs(
        &mut server,
        vec![
            workspace_folder(Uri::from_file_path(&project_a).unwrap(), "a"),
            workspace_folder(Uri::from_file_path(&project_b).unwrap(), "b"),
        ],
        vec![
            json!([{ "lintConfigurationFilePath": "a-config.json" }]),
            json!([{ "lintConfigurationFilePath": "b-config.json" }]),
        ],
    )
    .await;

    server
        .send_request(build_did_open(&uri_a, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_a = server.receive_notification().await;
    assert_eq!(
        diag_a.params().unwrap()["uri"].as_str(),
        Some(uri_a.as_str())
    );
    assert!(
        diag_a.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "project A's explicit override disables no-distinct, so it must be used over the default file"
    );

    server
        .send_request(build_did_open(&uri_b, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_b = server.receive_notification().await;
    assert_eq!(
        diag_b.params().unwrap()["uri"].as_str(),
        Some(uri_b.as_str())
    );
    assert!(
        !diag_b.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty(),
        "project B must resolve its own override path, not the first root's"
    );
}

#[tokio::test]
async fn diagnostics_are_empty_when_lint_config_is_missing() {
    let mut server = new_test_server();
    let root_dir = unique_temp_dir("uroborosql-lsp-diag-no-config");
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("no-config.sql")).unwrap();

    initialize_server_with_root_uri(&mut server, root_uri, None).await;

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;", 1))
        .await;
    let diag_notification = server.receive_notification().await;
    assert_eq!(diag_notification.method(), PublishDiagnostics::METHOD);
    assert!(
        diag_notification.params().unwrap()["diagnostics"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
