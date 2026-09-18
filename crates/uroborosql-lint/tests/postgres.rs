#![cfg(feature = "postgres-catalog")]

use sqlx::{postgres::PgConnectOptions, Connection, PgConnection};
use std::{env, time::Duration};
use uroborosql_lint::catalog::{
    postgres::{CatalogTimeouts, PostgresCatalogProvider, PostgresConfig, TlsMode},
    AbsenceKind, AcquisitionDetail, AcquisitionErrorKind, CatalogProvider, ConfigurationField,
    Lookup, TableDefinition, TableRequest, TimeoutScope, UnknownReason,
};

fn config() -> PostgresConfig {
    let mut config = PostgresConfig::new("127.0.0.1", "postgres", "postgres");
    config.port = env::var("CATALOG_TEST_PORT")
        .expect("run tests/postgres/run.py")
        .parse()
        .unwrap();
    config.password = Some(env::var("CATALOG_TEST_PASSWORD").unwrap());
    config.tls_mode = TlsMode::Disable;
    config
}

fn request(schema: Option<&str>, name: &str) -> TableRequest {
    TableRequest {
        schema: schema.map(str::to_owned),
        name: name.to_owned(),
    }
}

fn found(value: Lookup<&TableDefinition>) -> &TableDefinition {
    match value {
        Lookup::Found(table) => table,
        other => panic!("expected found, got {other:?}"),
    }
}

fn names(table: &TableDefinition) -> Vec<&str> {
    table
        .columns
        .iter()
        .map(|column| column.name.as_str())
        .collect()
}

async fn admin() -> PgConnection {
    let config = config();
    PgConnection::connect_with(
        &PgConnectOptions::new_without_pgpass()
            .host(&config.host)
            .port(config.port)
            .username(&config.user)
            .database(&config.dbname)
            .password(config.password.as_ref().unwrap())
            .ssl_mode(sqlx::postgres::PgSslMode::Disable),
    )
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires isolated fixture; use tests/postgres/run.py"]
async fn definitions_and_visibility() {
    let requests = [
        request(None, "users"),
        request(Some("public"), "users"),
        request(Some("public"), "Users"),
        request(Some("public"), "dropped"),
        request(Some("public"), "zero_columns"),
        request(Some("public"), "partitioned"),
        request(Some("public"), "user_view"),
        request(Some("public"), "user_sequence"),
        request(Some("public"), "missing"),
        request(Some("missing"), "users"),
        request(Some("hidden"), "users"),
        request(Some("pg_temp"), "users"),
        request(Some("PG_TEMP"), "users"),
        request(Some("pg_temp_data"), "users"),
        request(None, "pg_class"),
        request(Some("public' OR true --"), "users"),
        request(None, "only_public"),
        request(None, "shadowed_view"),
    ];
    let mut config = config();
    config.user = "catalog_reader".into();
    let provider = PostgresCatalogProvider::new(config);
    let snapshot = provider.acquire(&requests).await.unwrap();
    assert_eq!(
        snapshot.effective_search_path(),
        &["pg_catalog", "app", "public"]
    );
    assert_eq!(names(found(snapshot.lookup(&requests[0]))), ["app_id"]);
    assert_eq!(
        names(found(snapshot.lookup(&requests[1]))),
        ["id", "name", "age"]
    );
    assert_eq!(names(found(snapshot.lookup(&requests[2]))), ["Id"]);
    assert_eq!(
        names(found(snapshot.lookup(&requests[3]))),
        ["first_col", "last_col"]
    );
    assert!(names(found(snapshot.lookup(&requests[4]))).is_empty());
    assert_eq!(names(found(snapshot.lookup(&requests[5]))), ["id"]);
    for index in [6, 7] {
        assert_eq!(
            snapshot.lookup(&requests[index]),
            Lookup::Unknown(UnknownReason::UnsupportedRelation)
        );
    }
    assert_eq!(
        snapshot.lookup(&requests[8]),
        Lookup::Absent(AbsenceKind::Table)
    );
    assert_eq!(
        snapshot.lookup(&requests[9]),
        Lookup::Absent(AbsenceKind::Schema)
    );
    assert!(
        matches!(snapshot.lookup(&requests[10]), Lookup::Unavailable(e) if e.kind == AcquisitionErrorKind::PermissionDenied && e.to_string().contains("USAGE privilege"))
    );
    assert_eq!(
        snapshot.lookup(&requests[11]),
        Lookup::Unknown(UnknownReason::UnsupportedSyntax)
    );
    assert_eq!(names(found(snapshot.lookup(&requests[12]))), ["quoted_id"]);
    assert_eq!(
        snapshot.lookup(&requests[13]),
        Lookup::Absent(AbsenceKind::Table)
    );
    assert_eq!(found(snapshot.lookup(&requests[14])).schema, "pg_catalog");
    assert_eq!(
        snapshot.lookup(&requests[15]),
        Lookup::Absent(AbsenceKind::Schema)
    );
    assert!(matches!(
        snapshot.lookup(&request(Some("public"), "not_requested")),
        Lookup::Unknown(UnknownReason::IncompleteCoverage)
    ));
    assert_eq!(found(snapshot.lookup(&requests[16])).schema, "public");
    assert_eq!(
        snapshot.lookup(&requests[17]),
        Lookup::Unknown(UnknownReason::UnsupportedRelation)
    );
    let table = found(snapshot.lookup(&requests[1]));
    let mut system: Vec<_> = table
        .system_columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    system.sort();
    assert_eq!(system, ["cmax", "cmin", "ctid", "tableoid", "xmax", "xmin"]);
    assert_eq!(table.column("missing"), Lookup::Absent(AbsenceKind::Column));
    assert_eq!(table.column("app_id"), Lookup::Absent(AbsenceKind::Column));
}

#[tokio::test]
#[ignore = "requires isolated fixture"]
async fn refreshes_each_acquisition() {
    let mut admin = admin().await;
    sqlx::query("CREATE TABLE public.changing (before_column integer)")
        .execute(&mut admin)
        .await
        .unwrap();
    let provider = PostgresCatalogProvider::new(config());
    let requests = [request(Some("public"), "changing")];
    let before = provider.acquire(&requests).await.unwrap();
    sqlx::query("ALTER TABLE public.changing RENAME COLUMN before_column TO after_column")
        .execute(&mut admin)
        .await
        .unwrap();
    let after = provider.acquire(&requests).await.unwrap();
    assert_eq!(names(found(before.lookup(&requests[0]))), ["before_column"]);
    assert_eq!(names(found(after.lookup(&requests[0]))), ["after_column"]);
    sqlx::query("DROP TABLE public.changing")
        .execute(&mut admin)
        .await
        .unwrap();
    admin.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated fixture"]
async fn failures_are_not_absence() {
    let mut config = config();
    config.password = Some("wrong-secret-never-in-error".into());
    let err = PostgresCatalogProvider::new(config)
        .acquire(&[])
        .await
        .unwrap_err();
    assert_eq!(err.kind, AcquisitionErrorKind::Connection);
    assert_eq!(err.detail, Some(AcquisitionDetail::Authentication));
    assert!(err
        .to_string()
        .contains("user/password and server authentication settings"));
    assert!(!format!("{err:?} {err}").contains("wrong-secret"));
    let mut config = self::config();
    config.dbname = "catalog_database_that_does_not_exist".into();
    let err = PostgresCatalogProvider::new(config)
        .acquire(&[])
        .await
        .unwrap_err();
    assert_eq!(err.detail, Some(AcquisitionDetail::DatabaseNotFound));
    assert!(err.to_string().contains("Check dbname"));
    let mut config = self::config();
    config.host = "/tmp".into();
    assert_eq!(
        PostgresCatalogProvider::new(config)
            .acquire(&[])
            .await
            .unwrap_err()
            .kind,
        AcquisitionErrorKind::InvalidData
    );
}

#[tokio::test]
#[ignore = "requires isolated fixture; environment set by parent process"]
async fn environment_is_explicitly_controlled() {
    if env::var("CATALOG_TEST_ENV").is_err() {
        return;
    }
    let mut config = config();
    if env::var("CATALOG_TEST_ENV").as_deref() == Ok("fallback") {
        config.password = None;
    }
    let requests = [request(None, "users")];
    let snapshot = PostgresCatalogProvider::new(config)
        .acquire(&requests)
        .await
        .unwrap();
    assert_eq!(
        snapshot.effective_search_path(),
        &["pg_catalog", "app", "public"]
    );
    assert_eq!(names(found(snapshot.lookup(&requests[0]))), ["app_id"]);
}

#[tokio::test]
#[ignore = "requires isolated fixture"]
async fn query_timeout_and_cancellation_close_connections() {
    let mut blocker = admin().await;
    sqlx::query("ALTER FUNCTION pg_catalog.has_schema_privilege(oid,text) RENAME TO catalog_test_original_privilege")
        .execute(&mut blocker).await.unwrap();
    sqlx::query("CREATE FUNCTION pg_catalog.has_schema_privilege(oid,text) RETURNS boolean LANGUAGE sql AS 'SELECT true FROM pg_catalog.pg_advisory_xact_lock(81726355)'")
        .execute(&mut blocker).await.unwrap();
    let mut monitor = admin().await;
    const WAITING_CATALOG_QUERIES: &str = "SELECT pid FROM pg_catalog.pg_stat_activity WHERE datname = pg_catalog.current_database() AND state = 'active' AND wait_event_type = 'Lock' AND wait_event = 'advisory' AND query LIKE '%pg_catalog.pg_class%'";
    // Delay the provider query without also blocking connection startup/catalog type discovery.
    sqlx::query_as::<_, (i32,)>(WAITING_CATALOG_QUERIES)
        .fetch_all(&mut monitor)
        .await
        .unwrap();
    sqlx::query("BEGIN").execute(&mut blocker).await.unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_xact_lock(81726355)")
        .execute(&mut blocker)
        .await
        .unwrap();
    let mut config = config();
    config.timeouts.query = Duration::from_secs(1);
    let provider = PostgresCatalogProvider::new(config);
    let request = [request(Some("public"), "users")];
    let start = tokio::time::Instant::now();
    let err = provider.acquire(&request).await.unwrap_err();
    assert_eq!(err.kind, AcquisitionErrorKind::Timeout);
    assert_eq!(
        err.detail,
        Some(AcquisitionDetail::Timeout {
            scope: TimeoutScope::Query,
            limit: Duration::from_secs(1),
        })
    );
    assert!(err.to_string().contains("database operation timed out"));
    assert_eq!(
        err.phase,
        uroborosql_lint::catalog::AcquisitionPhase::Relation
    );
    assert!(start.elapsed() >= Duration::from_secs(1));
    assert!(start.elapsed() < Duration::from_secs(4));
    let mut pending = sqlx::query_as::<_, (i32,)>(WAITING_CATALOG_QUERIES)
        .fetch_all(&mut monitor)
        .await
        .unwrap();
    let provider = PostgresCatalogProvider::new(self::config());
    let acquisition = tokio::spawn(async move { provider.acquire(&request).await });
    let cancelled_pid = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let rows = sqlx::query_as::<_, (i32,)>(WAITING_CATALOG_QUERIES)
                .fetch_all(&mut monitor)
                .await
                .unwrap();
            if let Some((pid,)) = rows.into_iter().find(|row| !pending.contains(row)) {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("acquisition did not reach the blocked catalog query");
    acquisition.abort();
    assert!(acquisition.await.unwrap_err().is_cancelled());
    pending.push((cancelled_pid,));
    sqlx::query("ROLLBACK").execute(&mut blocker).await.unwrap();
    // Socket drop does not proactively cancel the server query; observe exit after the lock releases.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let pids: Vec<i32> = pending.iter().map(|(pid,)| *pid).collect();
            let alive: (bool,) = sqlx::query_as(
                "SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE pid = ANY($1))",
            )
            .bind(&pids)
            .fetch_one(&mut monitor)
            .await
            .unwrap();
            if !alive.0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("acquisition backends remained after releasing the blocking lock");
    sqlx::query("DROP FUNCTION pg_catalog.has_schema_privilege(oid,text)")
        .execute(&mut blocker)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.catalog_test_original_privilege(oid,text) RENAME TO has_schema_privilege")
        .execute(&mut blocker).await.unwrap();
    monitor.close().await.unwrap();
    blocker.close().await.unwrap();
}

#[tokio::test]
async fn zero_timeouts_are_rejected_before_connecting() {
    for (timeouts, field) in [
        (
            CatalogTimeouts {
                connect: Duration::ZERO,
                ..CatalogTimeouts::default()
            },
            ConfigurationField::ConnectTimeout,
        ),
        (
            CatalogTimeouts {
                query: Duration::ZERO,
                ..CatalogTimeouts::default()
            },
            ConfigurationField::QueryTimeout,
        ),
        (
            CatalogTimeouts {
                acquisition: Duration::ZERO,
                ..CatalogTimeouts::default()
            },
            ConfigurationField::AcquisitionTimeout,
        ),
    ] {
        let mut config = PostgresConfig::new("localhost", "unused", "unused");
        config.timeouts = timeouts;
        let error = PostgresCatalogProvider::new(config)
            .acquire(&[])
            .await
            .unwrap_err();
        assert_eq!(error.kind, AcquisitionErrorKind::InvalidData);
        assert_eq!(
            error.detail,
            Some(AcquisitionDetail::InvalidConfiguration(field))
        );
        assert!(error.to_string().contains("positive duration"));
    }
}

#[tokio::test]
async fn unresponsive_connection_times_out_and_closes_socket() {
    use tokio::io::AsyncReadExt;
    // Both the connection limit and the outer acquisition limit must stop
    // an unresponsive peer, and the socket must close in either case.
    for (connect, acquisition, scope) in [
        (
            Duration::from_millis(250),
            Duration::from_secs(5),
            TimeoutScope::Connect,
        ),
        (
            Duration::from_secs(5),
            Duration::from_millis(250),
            TimeoutScope::Acquisition,
        ),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            assert!(!bytes.is_empty());
        });
        let mut config = PostgresConfig::new("127.0.0.1", "unused", "unused");
        config.port = port;
        config.tls_mode = TlsMode::Disable;
        config.timeouts.connect = connect;
        config.timeouts.acquisition = acquisition;
        let start = tokio::time::Instant::now();
        let error = PostgresCatalogProvider::new(config)
            .acquire(&[])
            .await
            .unwrap_err();
        assert_eq!(error.kind, AcquisitionErrorKind::Timeout);
        assert_eq!(
            error.detail,
            Some(AcquisitionDetail::Timeout {
                scope,
                limit: Duration::from_millis(250),
            })
        );
        assert!(start.elapsed() >= Duration::from_millis(250));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert_eq!(
            error.phase,
            uroborosql_lint::catalog::AcquisitionPhase::Connect
        );
        tokio::time::timeout(Duration::from_secs(1), peer)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires isolated fixture with permission to replace a catalog function"]
async fn total_deadline_limits_multiple_successful_queries() {
    let mut admin = admin().await;
    // Built-in OIDs dispatch directly to C; a distinct function OID is needed for this delay fixture.
    sqlx::query("ALTER FUNCTION pg_catalog.has_schema_privilege(oid,text) RENAME TO catalog_test_original_privilege")
        .execute(&mut admin).await.unwrap();
    sqlx::query("CREATE FUNCTION pg_catalog.has_schema_privilege(oid,text) RETURNS boolean LANGUAGE sql AS 'SELECT true FROM pg_catalog.pg_sleep(4)'")
        .execute(&mut admin).await.unwrap();
    let requests = [
        request(Some("public"), "users"),
        request(Some("public"), "dropped"),
        request(Some("public"), "zero_columns"),
    ];
    let provider = PostgresCatalogProvider::new(config());
    let start = tokio::time::Instant::now();
    let result = provider.acquire(&requests).await;
    let elapsed = start.elapsed();
    let mut extended = config();
    extended.timeouts.acquisition = Duration::from_secs(30);
    let extended_result = PostgresCatalogProvider::new(extended)
        .acquire(&requests)
        .await;
    sqlx::query("DROP FUNCTION pg_catalog.has_schema_privilege(oid,text)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.catalog_test_original_privilege(oid,text) RENAME TO has_schema_privilege")
        .execute(&mut admin).await.unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind, AcquisitionErrorKind::Timeout);
    assert_eq!(
        error.detail,
        Some(AcquisitionDetail::Timeout {
            scope: TimeoutScope::Acquisition,
            limit: Duration::from_secs(10),
        })
    );
    assert!(error.to_string().contains("overall acquisition timed out"));
    assert!(elapsed >= Duration::from_secs(10));
    assert!(elapsed < Duration::from_secs(14));
    let snapshot = extended_result.unwrap();
    for request in &requests {
        assert!(matches!(snapshot.lookup(request), Lookup::Found(_)));
    }
    admin.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated fixture"]
async fn catalog_permission_failure_is_unavailable() {
    let mut admin = admin().await;
    sqlx::query("REVOKE SELECT ON pg_catalog.pg_attribute FROM PUBLIC")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut config = config();
    config.user = "catalog_reader".into();
    let result = PostgresCatalogProvider::new(config)
        .acquire(&[request(Some("public"), "users")])
        .await;
    sqlx::query("GRANT SELECT ON pg_catalog.pg_attribute TO PUBLIC")
        .execute(&mut admin)
        .await
        .unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind, AcquisitionErrorKind::PermissionDenied);
    assert!(error.to_string().contains("catalog read privileges"));
    admin.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated fixture"]
async fn concurrent_ddl_keeps_one_snapshot() {
    let mut admin = admin().await;
    sqlx::query("CREATE TABLE public.concurrent_table (original_column integer)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.has_schema_privilege(oid,text) RENAME TO catalog_test_original_privilege")
        .execute(&mut admin).await.unwrap();
    sqlx::query("CREATE FUNCTION pg_catalog.has_schema_privilege(oid,text) RETURNS boolean LANGUAGE sql AS 'SELECT true FROM pg_catalog.pg_advisory_xact_lock(81726354)'")
        .execute(&mut admin).await.unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_lock(81726354)")
        .execute(&mut admin)
        .await
        .unwrap();
    let provider = PostgresCatalogProvider::new(config());
    let acquisition = tokio::spawn(async move {
        provider
            .acquire(&[request(Some("public"), "concurrent_table")])
            .await
    });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: (bool,) = sqlx::query_as("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE wait_event = 'advisory' AND datname = pg_catalog.current_database())")
                .fetch_one(&mut admin).await.unwrap();
            if waiting.0 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    sqlx::query("ALTER TABLE public.concurrent_table ADD COLUMN later_column integer")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_unlock(81726354)")
        .execute(&mut admin)
        .await
        .unwrap();
    let snapshot = acquisition.await.unwrap().unwrap();
    sqlx::query("DROP FUNCTION pg_catalog.has_schema_privilege(oid,text)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.catalog_test_original_privilege(oid,text) RENAME TO has_schema_privilege")
        .execute(&mut admin).await.unwrap();
    sqlx::query("DROP TABLE public.concurrent_table")
        .execute(&mut admin)
        .await
        .unwrap();
    assert_eq!(
        names(found(
            snapshot.lookup(&request(Some("public"), "concurrent_table"))
        )),
        ["original_column"]
    );
    admin.close().await.unwrap();
}

#[tokio::test]
#[ignore = "requires isolated fixture; parent supplies a valid pgpass"]
async fn pgpass_is_not_used() {
    let mut config = config();
    config.password = None;
    assert_eq!(
        PostgresCatalogProvider::new(config)
            .acquire(&[])
            .await
            .unwrap_err()
            .kind,
        AcquisitionErrorKind::Connection
    );
}
