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

#[tokio::test]
async fn format_selections_as_sql_returns_edits_in_order() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///host.ts").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_format_selections_as_sql(
            &uri,
            7,
            json!([
                {
                    "range": {
                        "start": { "line": 1, "character": 2 },
                        "end": { "line": 1, "character": 18 }
                    },
                    "text": "select a from b"
                },
                {
                    "range": {
                        "start": { "line": 3, "character": 4 },
                        "end": { "line": 3, "character": 20 }
                    },
                    "text": "select c from d"
                }
            ]),
            10,
        ))
        .await;
    let response = server.receive_response().await;
    assert!(response.is_ok());

    let value = serde_json::to_value(&response).unwrap();
    assert_eq!(value["result"]["hostDocumentVersion"], 7);
    let edits = value["result"]["edits"]
        .as_array()
        .expect("custom formatting result should return edits");
    assert_eq!(edits.len(), 2);
    assert_eq!(edits[0]["range"]["start"]["line"], 1);
    assert_eq!(edits[1]["range"]["start"]["line"], 3);
    assert_ne!(edits[0]["newText"].as_str().unwrap(), "select a from b");
    assert_ne!(edits[1]["newText"].as_str().unwrap(), "select c from d");
}

#[tokio::test]
async fn format_selections_as_sql_requests_scoped_configuration_for_host_document_uri() {
    let mut server = new_test_server();
    let root_dir = unique_temp_dir("uroborosql-lsp-embedded-sql-scope");
    let root_uri = Uri::from_file_path(&root_dir).unwrap();
    let uri = Uri::from_file_path(root_dir.join("src").join("host.ts")).unwrap();

    initialize_server_with_root_uri(&mut server, root_uri, None).await;
    server.push_workspace_configuration_response(json!([
        {
            "keywordCase": "upper"
        }
    ]));

    server
        .send_request(build_format_selections_as_sql(
            &uri,
            1,
            json!([
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 16 }
                    },
                    "text": "select a from b"
                }
            ]),
            11,
        ))
        .await;

    let config_request = server.receive_server_request().await;
    assert_eq!(config_request.method(), "workspace/configuration");
    let config_value = serde_json::to_value(&config_request).unwrap();
    assert_eq!(
        config_value["params"]["items"][0]["scopeUri"],
        serde_json::Value::String(uri.to_string())
    );

    let response = server.receive_response().await;
    assert!(response.is_ok());
    let value = serde_json::to_value(&response).unwrap();
    let new_text = value["result"]["edits"][0]["newText"]
        .as_str()
        .expect("text edit should contain newText");
    assert!(new_text.starts_with("SELECT\n"));
}

#[tokio::test]
async fn format_selections_as_sql_fails_when_any_selection_fails() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///host.ts").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_format_selections_as_sql(
            &uri,
            3,
            json!([
                {
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 8 }
                    },
                    "text": "select 1"
                },
                {
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 3 }
                    },
                    "text": "!!!"
                }
            ]),
            12,
        ))
        .await;
    let response = server.receive_response().await;
    assert!(response.is_error());
}

#[tokio::test]
async fn format_selections_as_sql_rejects_empty_selection_list() {
    let mut server = new_test_server();
    let uri = Uri::from_str("file:///host.ts").unwrap();

    initialize_server(&mut server).await;

    server
        .send_request(build_format_selections_as_sql(&uri, 5, json!([]), 13))
        .await;
    let response = server.receive_response().await;
    assert!(response.is_error());
}
