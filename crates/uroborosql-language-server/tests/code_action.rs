mod test_harness;

use serde_json::Value;
use tower_lsp_server::UriExt;
use tower_lsp_server::lsp_types::notification::{Notification, PublishDiagnostics};
use tower_lsp_server::lsp_types::*;

use test_harness::*;

#[tokio::test]
async fn initialize_declares_quickfix_code_action_provider() {
    let mut server = new_test_server();

    server.send_request(build_initialize(1)).await;
    let response = server.receive_response().await;
    let provider = &response.result().unwrap()["capabilities"]["codeActionProvider"];

    assert_eq!(provider["codeActionKinds"][0].as_str(), Some("quickfix"));
}

#[tokio::test]
async fn lint_diagnostic_returns_disable_next_line_quickfix() {
    let mut server = new_test_server();
    let root_dir = initialize_server_with_default_lint_config(
        &mut server,
        "uroborosql-lsp-code-action-disable",
    )
    .await;
    let uri = Uri::from_file_path(root_dir.join("code-action.sql")).unwrap();

    server
        .send_request(build_did_open(
            &uri,
            "    SELECT DISTINCT id FROM users;\n",
            1,
        ))
        .await;
    let diagnostics = receive_diagnostics(&mut server).await;
    let diagnostic = diagnostic_with_code(&diagnostics, "no-distinct");

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            Some(vec![CodeActionKind::QUICKFIX]),
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]["title"].as_str(),
        Some("Disable no-distinct for next line")
    );
    assert_eq!(actions[0]["kind"].as_str(), Some("quickfix"));
    let edit = first_text_edit(&actions[0], &uri);
    assert_eq!(
        edit["newText"].as_str(),
        Some("    -- uroborosql-lint-disable-next-line no-distinct\n")
    );
    assert_eq!(edit["range"]["start"]["line"].as_u64(), Some(0));
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(0));
}

#[tokio::test]
async fn existing_disable_next_line_is_appended() {
    let mut server = new_test_server();
    let root_dir = initialize_server_with_default_lint_config(
        &mut server,
        "uroborosql-lsp-code-action-append",
    )
    .await;
    let uri = Uri::from_file_path(root_dir.join("append.sql")).unwrap();
    let text = "-- uroborosql-lint-disable-next-line no-distinct\nSELECT DISTINCT * FROM users;\n";
    let diagnostic = lint_diagnostic("no-wildcard-projection", 1, 16, 1, 17);

    server.send_request(build_did_open(&uri, text, 1)).await;
    let _ = server.receive_notification().await;

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            None,
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert_eq!(actions.len(), 1);
    let edit = first_text_edit(&actions[0], &uri);
    assert_eq!(edit["newText"].as_str(), Some(", no-wildcard-projection"));
    assert_eq!(edit["range"]["start"]["line"].as_u64(), Some(0));
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(48));
}

#[tokio::test]
async fn indented_existing_disable_next_line_is_appended() {
    let mut server = new_test_server();
    let root_dir = initialize_server_with_default_lint_config(
        &mut server,
        "uroborosql-lsp-code-action-append-indented",
    )
    .await;
    let uri = Uri::from_file_path(root_dir.join("append-indented.sql")).unwrap();
    let text =
        "    -- uroborosql-lint-disable-next-line no-distinct\n    SELECT DISTINCT * FROM users;\n";
    let diagnostic = lint_diagnostic("no-wildcard-projection", 1, 20, 1, 21);

    server.send_request(build_did_open(&uri, text, 1)).await;
    let _ = server.receive_notification().await;

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            None,
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert_eq!(actions.len(), 1);
    let edit = first_text_edit(&actions[0], &uri);
    assert_eq!(edit["newText"].as_str(), Some(", no-wildcard-projection"));
    assert_eq!(edit["range"]["start"]["line"].as_u64(), Some(0));
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(52));
}

#[tokio::test]
async fn unknown_rule_directive_returns_remove_quickfix() {
    let mut server = new_test_server();
    let root_dir = initialize_server_with_default_lint_config(
        &mut server,
        "uroborosql-lsp-code-action-unknown",
    )
    .await;
    let uri = Uri::from_file_path(root_dir.join("unknown-rule.sql")).unwrap();
    let text = "-- uroborosql-lint-disable-next-line no-distinct, definitely-not-a-rule\nSELECT DISTINCT id FROM users;\n";

    server.send_request(build_did_open(&uri, text, 1)).await;
    let diagnostics = receive_diagnostics(&mut server).await;
    let diagnostic = diagnostic_with_code(&diagnostics, "invalid-lint-directive");

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            None,
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]["title"].as_str(),
        Some("Remove unknown lint rule")
    );
    let edit = first_text_edit(&actions[0], &uri);
    assert_eq!(edit["newText"].as_str(), Some(""));
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(48));
    assert_eq!(edit["range"]["end"]["character"].as_u64(), Some(71));
}

#[tokio::test]
async fn indented_unknown_rule_directive_returns_remove_quickfix() {
    let mut server = new_test_server();
    let root_dir = initialize_server_with_default_lint_config(
        &mut server,
        "uroborosql-lsp-code-action-unknown-indented",
    )
    .await;
    let uri = Uri::from_file_path(root_dir.join("unknown-rule-indented.sql")).unwrap();
    let text = "    -- uroborosql-lint-disable-next-line no-distinct, definitely-not-a-rule\nSELECT DISTINCT id FROM users;\n";

    server.send_request(build_did_open(&uri, text, 1)).await;
    let diagnostics = receive_diagnostics(&mut server).await;
    let diagnostic = diagnostic_with_code(&diagnostics, "invalid-lint-directive");

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            None,
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert_eq!(actions.len(), 1);
    assert_eq!(
        actions[0]["title"].as_str(),
        Some("Remove unknown lint rule")
    );
    let edit = first_text_edit(&actions[0], &uri);
    assert_eq!(edit["newText"].as_str(), Some(""));
    assert_eq!(edit["range"]["start"]["character"].as_u64(), Some(52));
    assert_eq!(edit["range"]["end"]["character"].as_u64(), Some(75));
}

#[tokio::test]
async fn non_quickfix_only_returns_empty_actions() {
    let mut server = new_test_server();
    let root_dir =
        initialize_server_with_default_lint_config(&mut server, "uroborosql-lsp-code-action-only")
            .await;
    let uri = Uri::from_file_path(root_dir.join("only.sql")).unwrap();
    let diagnostic = lint_diagnostic("no-distinct", 0, 0, 0, 15);

    server
        .send_request(build_did_open(&uri, "SELECT DISTINCT id FROM users;\n", 1))
        .await;
    let _ = server.receive_notification().await;

    server
        .send_request(build_code_action(
            &uri,
            diagnostic.range,
            vec![diagnostic],
            Some(vec![CodeActionKind::REFACTOR]),
            2,
        ))
        .await;
    let actions = code_action_result(server.receive_response().await);

    assert!(actions.is_empty());
}

async fn receive_diagnostics(server: &mut TestServer) -> Vec<Diagnostic> {
    let notification = server.receive_notification().await;
    assert_eq!(notification.method(), PublishDiagnostics::METHOD);
    serde_json::from_value(notification.params().unwrap()["diagnostics"].clone()).unwrap()
}

fn diagnostic_with_code(diagnostics: &[Diagnostic], code: &str) -> Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(diagnostic_code)) if diagnostic_code == code
            )
        })
        .cloned()
        .expect("diagnostic with code")
}

fn lint_diagnostic(
    code: &str,
    start_line: u32,
    start_char: u32,
    end_line: u32,
    end_char: u32,
) -> Diagnostic {
    Diagnostic {
        range: Range::new(
            Position::new(start_line, start_char),
            Position::new(end_line, end_char),
        ),
        source: Some("uroborosql-lint".into()),
        code: Some(NumberOrString::String(code.into())),
        ..Diagnostic::default()
    }
}

fn code_action_result(response: tower_lsp_server::jsonrpc::Response) -> Vec<Value> {
    response.result().unwrap().as_array().unwrap().clone()
}

fn first_text_edit<'a>(action: &'a Value, uri: &Uri) -> &'a Value {
    &action["edit"]["changes"][uri.to_string()][0]
}
