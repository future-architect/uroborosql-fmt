mod test_harness;

use std::str::FromStr;

use tower_lsp_server::lsp_types::{Position, Range, Uri};

use test_harness::*;

#[tokio::test]
async fn utf16_range_change_should_not_corrupt_text() {
    let mut server = new_test_server();
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
    let mut server = new_test_server();
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
async fn utf16_range_formatting_returns_utf16_safe_range() {
    let mut server = new_test_server();
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
