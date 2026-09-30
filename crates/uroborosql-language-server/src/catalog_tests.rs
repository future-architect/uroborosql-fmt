use crate::{Backend, test_harness::*};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use tower_lsp_server::lsp_types::{
    DidChangeTextDocumentParams, TextDocumentContentChangeEvent, Uri,
    VersionedTextDocumentIdentifier,
};
use tower_lsp_server::{LanguageServer, UriExt};
use uroborosql_lint::catalog::*;

struct Delayed {
    started: mpsc::UnboundedSender<oneshot::Sender<()>>,
}
impl CatalogProvider for Delayed {
    fn acquire<'a>(&'a self, _: &'a [TableRequest]) -> AcquisitionFuture<'a> {
        Box::pin(async move {
            let (tx, rx) = oneshot::channel();
            self.started.send(tx).unwrap();
            let _ = rx.await;
            CatalogSnapshot::new(
                vec!["public".into()],
                vec![CatalogEntry {
                    schema: "public".into(),
                    table: "users".into(),
                    outcome: Lookup::Found(TableDefinition {
                        schema: "public".into(),
                        name: "users".into(),
                        columns: vec![ColumnDefinition { name: "id".into() }],
                        system_columns: vec![],
                    }),
                }],
            )
        })
    }
}
async fn setup() -> (
    TestServer,
    Backend,
    Uri,
    mpsc::UnboundedReceiver<oneshot::Sender<()>>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    let saved = Arc::new(Mutex::new(None));
    let capture = saved.clone();
    let mut server = TestServer::new(move |client| {
        let mut b = Backend::new(client);
        b.provider_factory = Some(Arc::new(move || {
            Box::new(Delayed {
                started: tx.clone(),
            })
        }));
        *capture.lock().unwrap() = Some(b.clone());
        b
    });
    let root = initialize_server_with_default_lint_config(&mut server, "lsp-catalog").await;
    let uri = Uri::from_file_path(root.join("query.sql")).unwrap();
    let backend = saved.lock().unwrap().clone().unwrap();
    (server, backend, uri, rx)
}
async fn open(b: &Backend, uri: &Uri, sql: &str) {
    b.did_open(tower_lsp_server::lsp_types::DidOpenTextDocumentParams {
        text_document: tower_lsp_server::lsp_types::TextDocumentItem::new(
            uri.clone(),
            "sql".into(),
            1,
            sql.into(),
        ),
    })
    .await;
}
async fn save(b: &Backend, uri: &Uri, sql: &str) {
    b.did_save(tower_lsp_server::lsp_types::DidSaveTextDocumentParams {
        text_document: tower_lsp_server::lsp_types::TextDocumentIdentifier::new(uri.clone()),
        text: Some(sql.into()),
    })
    .await;
}
async fn close(b: &Backend, uri: &Uri) {
    b.did_close(tower_lsp_server::lsp_types::DidCloseTextDocumentParams {
        text_document: tower_lsp_server::lsp_types::TextDocumentIdentifier::new(uri.clone()),
    })
    .await;
}

#[tokio::test]
async fn catalog_publication_coalesces_saves_and_retains_utf16_snapshot() {
    let (mut server, b, uri, mut started) = setup().await;
    open(&b, &uri, "SELECT old FROM users").await;
    let first = started.recv().await.unwrap();
    save(&b, &uri, "SELECT intermediate FROM users").await;
    save(&b, &uri, "SELECT '😀' AS label, missing FROM users").await;
    first.send(()).unwrap();
    started.recv().await.unwrap().send(()).unwrap();
    let notification = server.receive_notification().await;
    let values = notification.params().unwrap()["diagnostics"]
        .as_array()
        .unwrap();
    let d = values
        .iter()
        .find(|d| d["code"] == "no-unknown-reference")
        .unwrap();
    assert!(d["message"].as_str().unwrap().contains("missing"));
    assert_eq!(d["range"]["start"]["character"], 22);
    assert!(started.try_recv().is_err());
    b.stop_analysis().await;
}
#[tokio::test]
async fn close_and_reopen_same_version_never_publish_old_results() {
    let (mut server, b, uri, mut started) = setup().await;
    open(&b, &uri, "SELECT old FROM users").await;
    let first = started.recv().await.unwrap();
    close(&b, &uri).await;
    open(&b, &uri, "SELECT new FROM users").await;
    first.send(()).unwrap();
    started.recv().await.unwrap().send(()).unwrap();
    let clear = server.receive_notification().await;
    assert_eq!(
        clear.params().unwrap()["diagnostics"],
        serde_json::json!([])
    );
    let result = server.receive_notification().await;
    assert!(
        result.params().unwrap()["diagnostics"]
            .to_string()
            .contains("new")
    );
    assert!(
        !result.params().unwrap()["diagnostics"]
            .to_string()
            .contains("old")
    );
    b.stop_analysis().await;
}
#[tokio::test]
async fn change_invalidates_without_starting_another_acquisition() {
    let (_server, b, uri, mut started) = setup().await;
    open(&b, &uri, "SELECT old FROM users").await;
    let first = started.recv().await.unwrap();
    b.did_change(DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 2),
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "SELECT id FROM users".into(),
        }],
    })
    .await;
    first.send(()).unwrap();
    b.stop_analysis().await;
    assert!(started.try_recv().is_err());
}

#[tokio::test]
async fn configuration_path_changes_compose_with_file_events_and_stale_responses() {
    use tower_lsp_server::jsonrpc::Response;
    let (mut server, b, uri, _) = setup().await;
    server.auto_respond = false;
    let root = b.workspace_dir_for_uri(&uri).unwrap();
    write_file(&root.join("new.json"), r#"{"rules":{"no-distinct":"off"}}"#);
    let first_b = b.clone();
    let first = tokio::spawn(async move {
        first_b.refresh_workspace_configs().await;
    });
    let old = server.receive_server_request().await;
    b.rebuild_lint_config_stores().await;
    let next_b = b.clone();
    let next = tokio::spawn(async move {
        next_b.refresh_workspace_configs().await;
    });
    let new = server.receive_server_request().await;
    b.rebuild_lint_config_stores().await;
    server
        .send_response(Response::from_ok(
            new.id().unwrap().clone(),
            serde_json::json!([{"lintConfigurationFilePath":"new.json"}]),
        ))
        .await;
    next.await.unwrap();
    server
        .send_response(Response::from_ok(
            old.id().unwrap().clone(),
            serde_json::json!([{"lintConfigurationFilePath":"old.json"}]),
        ))
        .await;
    first.await.unwrap();
    let state = b.analysis.lock().await;
    let config = state.roots[&root]
        .store
        .as_ref()
        .unwrap()
        .resolve(&root.join("query.sql"));
    assert!(
        !b.linter
            .run("SELECT DISTINCT id FROM users", &config)
            .unwrap()
            .iter()
            .any(|d| d.code == "no-distinct")
    );
    assert_eq!(
        state.roots[&root]
            .config
            .as_ref()
            .unwrap()
            .lint_configuration_file_path
            .as_deref(),
        Some("new.json")
    );
    assert!(!state.roots[&root].pending);
    drop(state);
    b.stop_analysis().await;
}

#[tokio::test]
async fn configuration_timeout_is_root_local_and_late_reply_is_ignored() {
    use tower_lsp_server::jsonrpc::Response;
    let (mut server, b, uri, _) = setup().await;
    server.auto_respond = false;
    let first_root = b.workspace_dir_for_uri(&uri).unwrap();
    let second_root = unique_temp_dir("lsp-healthy-root");
    write_file(&second_root.join(".uroborosqllintrc.json"), "{}");
    let second_uri = Uri::from_file_path(&second_root).unwrap();
    b.workspace_roots
        .write()
        .unwrap()
        .push(crate::paths::WorkspaceRoot::from_uri(&second_uri).unwrap());
    let worker = b.clone();
    let refresh = tokio::spawn(async move {
        worker.refresh_workspace_configs().await;
    });
    let a = server.receive_server_request().await;
    let c = server.receive_server_request().await;
    let (healthy, late) = if a.params().unwrap()["items"][0]["scopeUri"] == second_uri.as_str() {
        (a, c)
    } else {
        (c, a)
    };
    server
        .send_response(Response::from_ok(
            healthy.id().unwrap().clone(),
            serde_json::json!([null]),
        ))
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if !b.analysis.lock().await.roots[&second_root].pending {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(b.analysis.lock().await.roots[&first_root].pending);
    refresh.await.unwrap();
    assert!(b.analysis.lock().await.roots[&first_root].unavailable);
    assert!(!b.analysis.lock().await.roots[&second_root].unavailable);
    server
        .send_response(Response::from_ok(
            late.id().unwrap().clone(),
            serde_json::json!([null]),
        ))
        .await;
    // A new request/response is a protocol barrier after the late response.
    server
        .send_request(
            tower_lsp_server::jsonrpc::Request::build("shutdown")
                .id(99)
                .finish(),
        )
        .await;
    assert!(server.receive_response().await.is_ok());
    assert!(b.analysis.lock().await.roots[&first_root].unavailable);
}

#[tokio::test]
async fn provider_failure_keeps_cst_and_status_is_ordered_after_diagnostics() {
    let (mut server, mut b, uri, _) = setup().await;
    b.provider_factory = Some(Arc::new(|| {
        Box::new(InMemoryCatalogProvider::failing(AcquisitionError::new(
            AcquisitionPhase::Connect,
            AcquisitionErrorKind::Connection,
        )))
    }));
    server.receive_logs = true;
    open(&b, &uri, "SELECT DISTINCT missing FROM users").await;
    let diagnostic = server.receive_notification().await;
    assert_eq!(diagnostic.method(), "textDocument/publishDiagnostics");
    let values = diagnostic.params().unwrap()["diagnostics"].to_string();
    assert!(values.contains("no-distinct"));
    assert!(!values.contains("no-unknown-reference"));
    let status = server.receive_notification().await;
    assert_eq!(status.method(), "window/logMessage");
    assert_eq!(status.params().unwrap()["type"], 1);
    b.stop_analysis().await;
}

#[tokio::test]
async fn shutdown_releases_blocked_publication_and_suppresses_later_sends() {
    let (mut server, b, uri, _) = setup().await;
    let worker = b.clone();
    let (tx, rx) = oneshot::channel();
    let sender = tokio::spawn(async move {
        let _state = worker.analysis.lock().await;
        tx.send(()).unwrap();
        for _ in 0..16 {
            worker
                .log_analysis(
                    tower_lsp_server::lsp_types::MessageType::INFO,
                    "x".repeat(16384),
                )
                .await;
        }
    });
    rx.await.unwrap();
    assert!(b.analysis.try_lock().is_err());
    tokio::time::timeout(std::time::Duration::from_secs(1), b.stop_analysis())
        .await
        .unwrap();
    sender.await.unwrap();
    assert!(b.analysis.lock().await.stopped);
    // No new notification may enter the transport after stop, even outside a worker.
    b.publish_analysis(uri, vec![], None).await;
    assert!(
        server
            .receive_notification_timeout(std::time::Duration::from_millis(20))
            .await
            .is_none()
    );
}
#[tokio::test]
async fn shutdown_aborts_unfinished_acquisition_after_deadline() {
    let (_server, b, uri, mut started) = setup().await;
    open(&b, &uri, "SELECT id FROM users").await;
    let held = started.recv().await.unwrap();
    let before = tokio::time::Instant::now();
    b.stop_analysis().await;
    assert!(held.is_closed());
    assert!(before.elapsed() < std::time::Duration::from_secs(6));
    assert_eq!(b.slots.available_permits(), 4);
}

#[tokio::test]
async fn catalog_acquisition_is_limited_to_four_documents() {
    let (_server, b, uri, mut started) = setup().await;
    let root = b.workspace_dir_for_uri(&uri).unwrap();
    for n in 0..5 {
        open(
            &b,
            &Uri::from_file_path(root.join(format!("{n}.sql"))).unwrap(),
            "SELECT id FROM users",
        )
        .await;
    }
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(started.recv().await.unwrap());
    }
    assert_eq!(b.slots.available_permits(), 0);
    assert!(started.try_recv().is_err());
    held.pop().unwrap().send(()).unwrap();
    held.push(started.recv().await.unwrap());
    b.stopping.send_replace(true);
    for sender in held {
        let _ = sender.send(());
    }
    b.stop_analysis().await;
}

#[tokio::test]
async fn latest_configuration_failure_clears_and_ignores_stale_success_and_analysis() {
    use tower_lsp_server::jsonrpc::Response;
    let (mut server, b, uri, mut started) = setup().await;
    open(&b, &uri, "SELECT obsolete FROM users").await;
    let held = started.recv().await.unwrap();
    server.auto_respond = false;
    let worker = b.clone();
    let first = tokio::spawn(async move {
        worker.refresh_workspace_configs().await;
    });
    let old = server.receive_server_request().await;
    let worker = b.clone();
    let second = tokio::spawn(async move {
        worker.refresh_workspace_configs().await;
    });
    let latest = server.receive_server_request().await;
    save(&b, &uri, "SELECT newer FROM users").await;
    server
        .send_response(Response::from_ok(
            latest.id().unwrap().clone(),
            serde_json::Value::Null,
        ))
        .await;
    second.await.unwrap();
    server
        .send_response(Response::from_ok(
            old.id().unwrap().clone(),
            serde_json::json!([null]),
        ))
        .await;
    first.await.unwrap();
    held.send(()).unwrap();
    let clear = server.receive_notification().await;
    assert_eq!(
        clear.params().unwrap()["diagnostics"],
        serde_json::json!([])
    );
    b.stop_analysis().await;
    assert!(started.try_recv().is_err());
    assert!(
        server
            .receive_notification_timeout(std::time::Duration::from_millis(20))
            .await
            .is_none()
    );
}
#[tokio::test]
async fn slot_wait_deadline_reports_deferred_without_acquiring() {
    let (mut server, b, uri, mut started) = setup().await;
    server.receive_logs = true;
    let permits = b.slots.acquire_many(4).await.unwrap();
    open(&b, &uri, "SELECT id FROM users").await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(11),
        server.receive_notification(),
    )
    .await
    .unwrap();
    assert_eq!(result.method(), "textDocument/publishDiagnostics");
    let result = server.receive_notification().await;
    assert_eq!(result.method(), "window/logMessage");
    assert!(
        result.params().unwrap()["message"]
            .as_str()
            .unwrap()
            .contains("deferred")
    );
    assert!(started.try_recv().is_err());
    drop(permits);
    b.stop_analysis().await;
}

#[tokio::test]
async fn file_change_during_store_build_forces_rebuild_before_application() {
    let (_server, mut b, uri, _) = setup().await;
    let root = b.workspace_dir_for_uri(&uri).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel::<oneshot::Sender<()>>();
    b.build_hook = Some(Arc::new(move || {
        let tx = tx.clone();
        Box::pin(async move {
            let (a, b) = oneshot::channel();
            tx.send(a).unwrap();
            let _ = b.await;
        })
    }));
    b.rebuild_lint_config_stores().await;
    let first = rx.recv().await.unwrap();
    write_file(
        &root.join(".uroborosqllintrc.json"),
        r#"{"rules":{"no-distinct":"off"}}"#,
    );
    b.rebuild_lint_config_stores().await;
    first.send(()).unwrap();
    let second = rx.recv().await.unwrap();
    assert!(b.analysis.lock().await.roots[&root].pending);
    second.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if !b.analysis.lock().await.roots[&root].pending {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let state = b.analysis.lock().await;
    let cfg = state.roots[&root]
        .store
        .as_ref()
        .unwrap()
        .resolve(&root.join("query.sql"));
    assert!(
        !b.linter
            .run("SELECT DISTINCT id FROM users", &cfg)
            .unwrap()
            .iter()
            .any(|d| d.code == "no-distinct")
    );
    drop(state);
    b.stop_analysis().await;
}

/// The fixture must contain public.users(id). Connection values stay outside logs.
#[cfg(feature = "postgres-catalog")]
#[tokio::test]
#[ignore = "requires LSP_TEST_PG_PORT and PostgreSQL fixture"]
async fn postgres_configuration_reaches_lsp_diagnostics() {
    let mut server = new_test_server();
    let root = unique_temp_dir("lsp-postgres");
    let port: u16 = std::env::var("LSP_TEST_PG_PORT")
        .expect("fixture port")
        .parse()
        .unwrap();
    write_file(&root.join(".uroborosqllintrc.json"),&serde_json::json!({"db":{"schemaProvider":"server","host":"127.0.0.1","port":port,"user":"postgres","password":"catalog-test","dbname":"postgres","tlsMode":"disable"}}).to_string());
    initialize_server_with_root_uri(&mut server, Uri::from_file_path(&root).unwrap(), None).await;
    let uri = Uri::from_file_path(root.join("query.sql")).unwrap();
    server
        .send_request(build_did_open(&uri, "SELECT missing FROM public.users", 1))
        .await;
    let result = server.receive_notification().await;
    let diagnostics = result.params().unwrap()["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "no-unknown-reference"
                && d["message"].as_str().unwrap().contains("missing"))
    );
    server
        .send_request(
            tower_lsp_server::jsonrpc::Request::build("shutdown")
                .id(99)
                .finish(),
        )
        .await;
    assert!(server.receive_response().await.is_ok());
}

/// Uses a real snapshot file, so offline checks are covered without PostgreSQL.
#[cfg(feature = "sqlite-catalog")]
#[tokio::test]
async fn sqlite_snapshot_configuration_reaches_lsp_diagnostics() {
    use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
    let mut server = new_test_server();
    let root = unique_temp_dir("lsp-sqlite");
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(root.join("catalog.sqlite"))
            .create_if_missing(true),
    )
    .await
    .unwrap();
    // The snapshot format is owned by uroborosql-lint; reuse its schema instead of copying it.
    sqlx::raw_sql(include_str!(
        "../../uroborosql-lint/src/catalog/sqlite/schema.sql"
    ))
    .execute(&mut connection)
    .await
    .unwrap();
    sqlx::raw_sql(
        "INSERT INTO pg_namespace VALUES (2200, 'public');
         INSERT INTO snapshot_schema_access VALUES (2200, 1);
         INSERT INTO snapshot_search_path VALUES (1, 2200);
         INSERT INTO pg_class VALUES (10, 2200, 'users', 'r', 1);
         INSERT INTO pg_attribute VALUES (10, 1, 'id', 0);
         INSERT INTO snapshot_meta VALUES (1, 1, 180000, 'db', 'login', 'role',
             '2026-09-18T00:00:00Z', 'database_catalog', 1, 1, 1, 1, 1);",
    )
    .execute(&mut connection)
    .await
    .unwrap();
    connection.close().await.unwrap();
    write_file(
        &root.join(".uroborosqllintrc.json"),
        &serde_json::json!({"db":{"schemaProvider":"file","path":"catalog.sqlite"}}).to_string(),
    );
    initialize_server_with_root_uri(&mut server, Uri::from_file_path(&root).unwrap(), None).await;
    let uri = Uri::from_file_path(root.join("query.sql")).unwrap();
    server
        .send_request(build_did_open(&uri, "SELECT id, missing FROM users", 1))
        .await;
    let result = server.receive_notification().await;
    let diagnostics = result.params().unwrap()["diagnostics"].as_array().unwrap();
    let unknown: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"] == "no-unknown-reference")
        .map(|d| d["message"].as_str().unwrap())
        .collect();
    assert_eq!(unknown.len(), 1, "{diagnostics:?}");
    assert!(unknown[0].contains("missing"), "{unknown:?}");
    server
        .send_request(
            tower_lsp_server::jsonrpc::Request::build("shutdown")
                .id(99)
                .finish(),
        )
        .await;
    assert!(server.receive_response().await.is_ok());
}

#[tokio::test]
async fn acquisition_free_diagnostics_bypass_saturated_slots() {
    let (mut server, mut b, uri, mut started) = setup().await;
    let slots = b.slots.clone();
    let held = slots.acquire_many(4).await.unwrap();
    // Parse errors and unsupported SELECTs must not enter CatalogProvider::acquire.
    for (sql, expected) in [
        ("SELECT ( FROM users", "Failed to parse SQL"),
        (
            "SELECT DISTINCT id FROM users JOIN others ON TRUE",
            "no-distinct",
        ),
    ] {
        open(&b, &uri, sql).await;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            server.receive_notification(),
        )
        .await
        .unwrap();
        assert!(
            result.params().unwrap()["diagnostics"]
                .to_string()
                .contains(expected)
        );
    }
    let root = b.workspace_dir_for_uri(&uri).unwrap();
    write_file(
        &root.join(".uroborosqllintrc.json"),
        r#"{"rules":{"no-unknown-reference":"off"}}"#,
    );
    b.rebuild_lint_config_stores().await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        server.receive_notification(),
    )
    .await
    .unwrap();
    assert!(
        result.params().unwrap()["diagnostics"]
            .to_string()
            .contains("no-distinct")
    );
    assert!(started.try_recv().is_err());
    // Unconfigured DB also runs the syntax rules without a slot.
    b.provider_factory = None;
    save(&b, &uri, "SELECT DISTINCT id FROM users").await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        server.receive_notification(),
    )
    .await
    .unwrap();
    assert!(
        result.params().unwrap()["diagnostics"]
            .to_string()
            .contains("no-distinct")
    );
    drop(held);
    b.stop_analysis().await;
}

#[cfg(feature = "postgres-catalog")]
#[tokio::test]
#[ignore = "requires LSP_TEST_PG_PORT and an isolated PostgreSQL fixture"]
async fn shutdown_releases_real_postgres_session_and_transaction() {
    use sqlx::{Connection, Executor};
    assert_eq!(
        std::env::var("LSP_TEST_DISPOSABLE_DATABASE").as_deref(),
        Ok("catalog-lsp-smoke-20260929"),
        "catalog mutation test requires an explicitly designated disposable local container; never use an existing database"
    );
    let port: u16 = std::env::var("LSP_TEST_PG_PORT").unwrap().parse().unwrap();
    let options = sqlx::postgres::PgConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("postgres")
        .password("catalog-test")
        .database("postgres")
        .ssl_mode(sqlx::postgres::PgSslMode::Disable);
    let mut observer = sqlx::PgConnection::connect_with(&options).await.unwrap();
    let mut locker = sqlx::PgConnection::connect_with(&options).await.unwrap();
    observer
        .execute("CREATE ROLE lsp_abort LOGIN PASSWORD 'catalog-test'")
        .await
        .unwrap();
    observer
        .execute("GRANT USAGE ON SCHEMA public TO lsp_abort")
        .await
        .unwrap();
    observer
        .execute("GRANT SELECT ON public.users TO lsp_abort")
        .await
        .unwrap();
    observer
        .execute("ALTER ROLE lsp_abort SET client_connection_check_interval='100ms'")
        .await
        .unwrap();
    // Isolated-fixture injection: pause the real metadata query inside its transaction,
    // without blocking connection initialization or the independent observer.
    observer.execute("ALTER FUNCTION pg_catalog.has_schema_privilege(oid,text) RENAME TO lsp_fixture_original_has_schema_privilege").await.unwrap();
    observer.execute("CREATE FUNCTION pg_catalog.has_schema_privilege(oid,text) RETURNS boolean LANGUAGE plpgsql VOLATILE AS $$ BEGIN PERFORM pg_catalog.pg_advisory_xact_lock(734271); RETURN true; END $$").await.unwrap();
    let (mut server, mut b, uri, _) = setup().await;
    b.provider_factory = None;
    let root = b.workspace_dir_for_uri(&uri).unwrap();
    write_file(&root.join(".uroborosqllintrc.json"),&serde_json::json!({"db":{"schemaProvider":"server","host":"127.0.0.1","port":port,"user":"lsp_abort","password":"catalog-test","dbname":"postgres","tlsMode":"disable","timeouts":{"connectMs":5000,"queryMs":60000,"acquisitionMs":60000}}}).to_string());
    b.rebuild_lint_config_stores().await;
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if !b.analysis.lock().await.roots[&root].pending {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // Prepare independent observations before starting the blocked acquisition.
    let _:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE usename='lsp_abort' AND wait_event_type='Lock' AND xact_start IS NOT NULL").fetch_one(&mut observer).await.unwrap();
    let _: i64 =
        sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE usename='lsp_abort'")
            .fetch_one(&mut observer)
            .await
            .unwrap();
    locker
        .execute("BEGIN; SELECT pg_advisory_xact_lock(734271)")
        .await
        .unwrap();
    let free: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(734271)")
        .fetch_one(&mut observer)
        .await
        .unwrap();
    assert!(!free, "fixture lock is not held");
    let function:String=sqlx::query_scalar("SELECT prosrc FROM pg_proc WHERE oid='pg_catalog.has_schema_privilege(oid,text)'::regprocedure").fetch_one(&mut observer).await.unwrap();
    assert!(function.contains("pg_advisory_xact_lock"));
    let mut probe = sqlx::PgConnection::connect_with(&options).await.unwrap();
    probe
        .execute("SET client_connection_check_interval='100ms'")
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(200),
            probe.execute(
                "SELECT pg_catalog.has_schema_privilege('public'::regnamespace::oid,'USAGE')"
            )
        )
        .await
        .is_err(),
        "fixture function did not block standalone SQL"
    );
    probe.close_hard().await.unwrap();
    open(&b, &uri, "SELECT missing FROM public.users").await;
    let observed=tokio::time::timeout(std::time::Duration::from_secs(10),async{loop{
        let count:i64=sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE usename='lsp_abort' AND wait_event_type='Lock' AND xact_start IS NOT NULL").fetch_one(&mut observer).await.unwrap();
        if count==1{break;}
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }}).await;
    assert!(
        observed.is_ok(),
        "provider did not reach blocked catalog acquisition"
    );
    let before = tokio::time::Instant::now();
    b.stop_analysis().await;
    assert!(
        before.elapsed() >= std::time::Duration::from_secs(5),
        "provider completed before forced cancellation"
    );
    let gone = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity WHERE usename='lsp_abort'",
            )
            .fetch_one(&mut observer)
            .await
            .unwrap();
            if count == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await;
    locker.execute("ROLLBACK").await.unwrap();
    assert!(
        gone.is_ok(),
        "provider session or transaction survived shutdown cancellation"
    );
    assert_eq!(b.slots.available_permits(), 4);
    assert!(
        server
            .receive_notification_timeout(std::time::Duration::from_millis(20))
            .await
            .is_none()
    );
    observer.execute("DROP FUNCTION pg_catalog.has_schema_privilege(oid,text); ALTER FUNCTION pg_catalog.lsp_fixture_original_has_schema_privilege(oid,text) RENAME TO has_schema_privilege").await.unwrap();
    observer
        .execute("DROP OWNED BY lsp_abort; DROP ROLE lsp_abort")
        .await
        .unwrap();
    locker.close().await.unwrap();
    observer.close().await.unwrap();
}
