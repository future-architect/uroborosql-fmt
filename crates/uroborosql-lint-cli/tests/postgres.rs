#![cfg(feature = "postgres-catalog")]
//! Executed by the existing disposable PostgreSQL fixture runner, never a user database.
use assert_cmd::Command;
use std::{env, fs};
use tempfile::tempdir;
use uroborosql_lint::{
    catalog::{
        postgres::{PostgresCatalogProvider, PostgresConfig, TlsMode},
        CatalogEntry, CatalogSnapshot, ColumnDefinition, InMemoryCatalogProvider, Lookup,
        TableDefinition,
    },
    Linter, ResolvedLintConfig,
};

#[tokio::test]
#[ignore = "requires disposable PostgreSQL fixture runner"]
async fn cli_and_public_api_use_postgres() {
    let port: u16 = env::var("CATALOG_TEST_PORT")
        .expect("fixture port")
        .parse()
        .unwrap();
    let password = env::var("CATALOG_TEST_PASSWORD").expect("fixture password");
    let dir = tempdir().unwrap();
    let config = dir.path().join("lint.json");
    let input = dir.path().join("query.sql");
    let write_config = |password: &str| {
        fs::write(&config, format!(r#"{{"db":{{"schemaProvider":"server","host":"127.0.0.1","port":{port},"user":"postgres","password":"{password}","dbname":"postgres","tlsMode":"disable","timeouts":{{"connectMs":5000,"queryMs":5000,"acquisitionMs":10000}}}}}}"#)).unwrap()
    };
    write_config(&password);
    fs::write(&input, "SELECT missing FROM public.users; SELECT id FROM public.missing_table; SELECT DISTINCT id FROM public.users;").unwrap();
    let run = |level: &str| {
        let mut command = Command::cargo_bin("uroborosql-lint").unwrap();
        command
            .current_dir(dir.path())
            .arg(&input)
            .arg("--config")
            .arg(&config)
            .args(["--fail-level", level]);
        command.output().unwrap()
    };
    let output = run("error");
    assert_eq!(output.status.code(), Some(1));
    let diagnostics = String::from_utf8(output.stdout).unwrap();
    assert_eq!(diagnostics.matches("no-unknown-reference").count(), 2);
    assert!(diagnostics.contains("query.sql:1:8: error: no-unknown-reference"));
    assert!(diagnostics.contains("no-distinct"));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("complete=2 excluded=1 failed=0"));
    assert_eq!(run("none").status.code(), Some(0));
    write_config("wrong-password");
    let output = run("none");
    assert_eq!(output.status.code(), Some(2));
    let diagnostics = String::from_utf8(output.stdout).unwrap();
    assert!(diagnostics.contains("no-distinct"));
    assert!(!diagnostics.contains("no-unknown-reference"));
    let status = String::from_utf8(output.stderr).unwrap();
    assert!(status.contains("authentication failed"));
    assert!(!status.contains(&password) && !status.contains("wrong-password"));

    let mut pg = PostgresConfig::new("127.0.0.1", "postgres", "postgres");
    pg.port = port;
    pg.password = Some(password);
    pg.tls_mode = TlsMode::Disable;
    let server = PostgresCatalogProvider::new(pg);
    let memory = InMemoryCatalogProvider::new(
        CatalogSnapshot::new(
            vec!["pg_catalog".into(), "public".into()],
            vec![CatalogEntry {
                schema: "public".into(),
                table: "users".into(),
                outcome: Lookup::Found(TableDefinition {
                    schema: "public".into(),
                    name: "users".into(),
                    columns: ["id", "name", "age"]
                        .into_iter()
                        .map(|name| ColumnDefinition { name: name.into() })
                        .collect(),
                    system_columns: vec![],
                }),
            }],
        )
        .unwrap(),
    );
    let cfg = ResolvedLintConfig::default();
    for sql in [
        "SELECT id, missing FROM public.users WHERE agge=1;",
        "SELECT DISTINCT id FROM public.users; SELECT name FROM public.users;",
        "SELECT x.id FROM /*#table*/ AS x; SELECT missing FROM public.users;",
    ] {
        let linter = Linter::new();
        let actual = linter.run_async(sql, &cfg, Some(&server)).await.unwrap();
        let expected = linter.run_async(sql, &cfg, Some(&memory)).await.unwrap();
        assert_eq!(actual.diagnostics, expected.diagnostics, "{sql}");
        assert_eq!(actual.catalog, expected.catalog, "{sql}");
    }
}
