#![cfg(all(feature = "postgres-catalog", feature = "sqlite-catalog"))]
use std::env;
use uroborosql_lint::catalog::{
    postgres::{PostgresCatalogProvider, PostgresConfig, TlsMode},
    sqlite::{
        export::{default_timeouts, export_catalog},
        SqliteCatalogProvider,
    },
    CatalogProvider, TableRequest,
};

fn config() -> PostgresConfig {
    let mut config = PostgresConfig::new("127.0.0.1", "catalog_reader", "postgres");
    config.port = env::var("CATALOG_TEST_PORT")
        .expect("use the PostgreSQL fixture runner")
        .parse()
        .unwrap();
    config.password = Some(env::var("CATALOG_TEST_PASSWORD").unwrap());
    config.tls_mode = TlsMode::Disable;
    config.timeouts = default_timeouts();
    config
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL fixture"]
async fn official_export_matches_live_provider() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("catalog.sqlite");
    let mut requests = Vec::new();
    for (schema, name) in [
        (None, "users"),
        (Some("public"), "users"),
        (Some("public"), "Users"),
        (Some("public"), "dropped"),
        (Some("public"), "zero_columns"),
        (Some("public"), "partitioned"),
        (Some("public"), "user_view"),
        (Some("public"), "user_sequence"),
        (Some("public"), "missing"),
        (Some("missing"), "users"),
        (Some("hidden"), "users"),
        (Some("pg_temp"), "users"),
        (Some("PG_TEMP"), "users"),
        (Some("pg_temp_data"), "users"),
        (None, "pg_class"),
        (Some("public' OR true --"), "users"),
        (None, "only_public"),
        (None, "shadowed_view"),
    ] {
        requests.push(TableRequest {
            schema: schema.map(str::to_owned),
            name: name.into(),
        });
    }
    let config = config();
    let live = PostgresCatalogProvider::new(config.clone())
        .acquire(&requests)
        .await
        .unwrap();
    export_catalog(&config, &path).await.unwrap();
    let offline = SqliteCatalogProvider::new(&path)
        .acquire(&requests)
        .await
        .unwrap();
    assert_eq!(live, offline);
    let memory = uroborosql_lint::catalog::InMemoryCatalogProvider::new(live);
    let server = PostgresCatalogProvider::new(config.clone());
    let file = SqliteCatalogProvider::new(&path);
    let linter = uroborosql_lint::Linter::new();
    let cfg = uroborosql_lint::ResolvedLintConfig::default();
    for sql in [
        "SELECT id, missing FROM public.users WHERE agge=1;",
        "SELECT ctid, first_col, removed, last_col FROM public.dropped;",
        "SELECT id FROM users; SELECT missing FROM public.user_view;",
        "SELECT missing FROM hidden.users; SELECT id FROM public.partitioned;",
        "SELECT x.id FROM /*#table*/ AS x; SELECT missing FROM public.users;",
        "SELECT missing FROM shadowed_view; SELECT \"Id\" FROM public.\"Users\";",
    ] {
        let expected = linter.run_async(sql, &cfg, Some(&server)).await.unwrap();
        for provider in [&memory as &dyn CatalogProvider, &file] {
            let actual = linter.run_async(sql, &cfg, Some(provider)).await.unwrap();
            assert_eq!(actual.diagnostics, expected.diagnostics, "{sql}");
            assert_eq!(actual.catalog, expected.catalog, "{sql}");
        }
    }
    // The portable artifact needs no journal or connection settings.
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    let bytes = std::fs::read(&path).unwrap();
    let mut invalid = config;
    invalid.host = "127.0.0.1".into();
    invalid.port = 1;
    assert!(export_catalog(&invalid, &path).await.is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL fixture; changes a catalog function"]
async fn export_is_consistent_during_ddl_and_query_timeout_preserves_output() {
    use sqlx::{
        postgres::{PgConnectOptions, PgSslMode},
        Connection, PgConnection,
    };
    use std::time::Duration;
    use uroborosql_lint::catalog::sqlite::export::ExportError;
    use uroborosql_lint::catalog::{AcquisitionErrorKind, Lookup};
    let cfg = config();
    let mut admin = PgConnection::connect_with(
        &PgConnectOptions::new_without_pgpass()
            .host(&cfg.host)
            .port(cfg.port)
            .username("postgres")
            .database(&cfg.dbname)
            .password(cfg.password.as_ref().unwrap())
            .ssl_mode(PgSslMode::Disable),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE public.export_concurrent (original_column integer)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.has_schema_privilege(oid,text) RENAME TO catalog_export_original_privilege").execute(&mut admin).await.unwrap();
    sqlx::query("CREATE FUNCTION pg_catalog.has_schema_privilege(oid,text) RETURNS boolean LANGUAGE sql AS 'SELECT true FROM pg_catalog.pg_advisory_xact_lock(81726355)'").execute(&mut admin).await.unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_lock(81726355)")
        .execute(&mut admin)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.sqlite");
    std::fs::write(&path, b"previous").unwrap();
    let output = path.clone();
    let exporting = cfg.clone();
    let acquisition = tokio::spawn(async move { export_catalog(&exporting, &output).await });
    tokio::time::timeout(Duration::from_secs(3),async {
        loop {
            let (waiting,):(bool,)=sqlx::query_as("SELECT EXISTS(SELECT 1 FROM pg_catalog.pg_stat_activity WHERE wait_event = 'advisory' AND datname = pg_catalog.current_database())").fetch_one(&mut admin).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    sqlx::query("ALTER TABLE public.export_concurrent ADD COLUMN later_column integer")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_unlock(81726355)")
        .execute(&mut admin)
        .await
        .unwrap();
    acquisition.await.unwrap().unwrap();
    // Run cancellation after the consistency check so the waiting backend belongs to this export.
    let previous = std::fs::read(&path).unwrap();
    sqlx::query("SELECT pg_catalog.pg_advisory_lock(81726355)")
        .execute(&mut admin)
        .await
        .unwrap();
    let mut limited = cfg;
    limited.timeouts.query = Duration::from_millis(30);
    let error = export_catalog(&limited, &path).await.unwrap_err();
    assert!(matches!(error,ExportError::Acquisition(e) if e.kind==AcquisitionErrorKind::Timeout));
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    sqlx::query("SELECT pg_catalog.pg_advisory_unlock(81726355)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION pg_catalog.has_schema_privilege(oid,text)")
        .execute(&mut admin)
        .await
        .unwrap();
    sqlx::query("ALTER FUNCTION pg_catalog.catalog_export_original_privilege(oid,text) RENAME TO has_schema_privilege").execute(&mut admin).await.unwrap();
    sqlx::query("DROP TABLE public.export_concurrent")
        .execute(&mut admin)
        .await
        .unwrap();
    admin.close().await.unwrap();
    let request = TableRequest {
        schema: Some("public".into()),
        name: "export_concurrent".into(),
    };
    let snapshot = SqliteCatalogProvider::new(&path)
        .acquire(std::slice::from_ref(&request))
        .await
        .unwrap();
    assert!(
        matches!(snapshot.lookup(&request),Lookup::Found(t) if t.columns.iter().map(|c|c.name.as_str()).collect::<Vec<_>>()==["original_column"])
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}
