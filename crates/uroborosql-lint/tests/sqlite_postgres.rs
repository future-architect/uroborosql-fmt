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
