mod test_harness;

use std::str::FromStr;

use serde_json::json;
use tower_lsp_server::UriExt;
use tower_lsp_server::lsp_types::{Position, Range, Uri};

use test_harness::*;

#[tokio::test]
async fn document_formatting_returns_edit() {
    let mut server = new_test_server();
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
    let mut server = new_test_server();
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
async fn formatting_merges_config_file_with_explicit_client_overrides() {
    let mut server = new_test_server();
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
    let mut server = new_test_server();
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
