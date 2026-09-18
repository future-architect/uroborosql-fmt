use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn export_requires_only_connection_arguments_and_rejects_lint_arguments() {
    for args in [
        vec!["export-catalog"],
        vec![
            "export-catalog",
            "--host",
            "host",
            "--user",
            "user",
            "--dbname",
            "db",
            "--connect-timeout-ms",
            "0",
        ],
        vec![
            "export-catalog",
            "--host",
            "host",
            "--user",
            "user",
            "--dbname",
            "db",
            "--query-timeout-ms",
            "-1",
        ],
        vec![
            "export-catalog",
            "--host",
            "host",
            "--user",
            "user",
            "--dbname",
            "db",
            "--config",
            "lint.json",
        ],
        vec![
            "export-catalog",
            "--host",
            "host",
            "--user",
            "user",
            "--dbname",
            "db",
            "query.sql",
        ],
        vec![
            "--init",
            "export-catalog",
            "--host",
            "host",
            "--user",
            "user",
            "--dbname",
            "db",
        ],
    ] {
        Command::cargo_bin("uroborosql-lint")
            .unwrap()
            .args(args)
            .assert()
            .code(2)
            .stdout("");
    }
}

#[cfg(all(feature = "postgres-catalog", feature = "sqlite-catalog"))]
#[test]
fn failed_export_ignores_lint_configuration_and_preserves_the_destination() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join(".uroborosqllintrc.json"), "invalid json").unwrap();
    let output = temp.path().join("catalog.sqlite");
    std::fs::write(&output, b"previous snapshot").unwrap();
    Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "export-catalog",
            "--host",
            "127.0.0.1",
            "--port",
            "1",
            "--user",
            "user",
            "--dbname",
            "db",
            "--tls-mode",
            "disable",
            "--output",
            "catalog.sqlite",
            "--connect-timeout-ms",
            "50",
        ])
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("Catalog export failed for catalog.sqlite"));
    assert_eq!(std::fs::read(output).unwrap(), b"previous snapshot");
}

#[test]
fn same_named_sql_file_is_still_accessible_by_path() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("export-catalog"),
        "SELECT DISTINCT id FROM users;",
    )
    .unwrap();
    std::fs::write(temp.path().join(".uroborosqllintrc.json"), "{}").unwrap();
    Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(temp.path())
        .arg("./export-catalog")
        .assert()
        .success()
        .stdout(contains("no-distinct"));
}

#[cfg(all(feature = "postgres-catalog", feature = "sqlite-catalog"))]
#[test]
#[ignore = "requires disposable PostgreSQL fixture runner"]
fn official_export_runs_without_config_and_offline_cli_matches_live() {
    use std::{env, fs};
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    fs::write(
        dir.join(".uroborosqllintrc.json"),
        "invalid config must not be read during export",
    )
    .unwrap();
    fs::create_dir(dir.join("config")).unwrap();
    let port = env::var("CATALOG_TEST_PORT").unwrap();
    let password = env::var("CATALOG_TEST_PASSWORD").unwrap();
    let mut export = Command::cargo_bin("uroborosql-lint").unwrap();
    export
        .current_dir(dir)
        .args([
            "export-catalog",
            "--host",
            "127.0.0.1",
            "--port",
            &port,
            "--user",
            "catalog_reader",
            "--dbname",
            "postgres",
            "--tls-mode",
            "disable",
            "--output",
            "config/catalog.sqlite",
        ])
        .env("PGPASSWORD", &password)
        .env("PGOPTIONS", "-c search_path=app,public")
        .env("PGHOST", "invalid")
        .env("PGPORT", "1")
        .env("PGUSER", "invalid")
        .env("PGDATABASE", "invalid")
        .env("PGSSLMODE", "require");
    export
        .assert()
        .success()
        .stdout("")
        .stderr(contains("Catalog exported to config/catalog.sqlite"));
    // Same explicit name overwrites a complete file.
    export.assert().success().stdout("");
    fs::write(dir.join("query.sql"),"SELECT missing FROM public.users; SELECT id FROM users; SELECT missing FROM public.user_view; SELECT DISTINCT id FROM public.users; SELECT x.id FROM /*#table*/ AS x;").unwrap();
    fs::write(dir.join("config/live.json"),format!(r#"{{"db":{{"schemaProvider":"server","host":"127.0.0.1","port":{port},"user":"catalog_reader","dbname":"postgres","tlsMode":"disable"}}}}"#)).unwrap();
    fs::write(
        dir.join("config/offline.json"),
        r#"{"db":{"schemaProvider":"file","path":"catalog.sqlite"}}"#,
    )
    .unwrap();
    let mut live = Command::cargo_bin("uroborosql-lint").unwrap();
    let live = live
        .current_dir(dir)
        .args(["query.sql", "--config", "config/live.json"])
        .env("PGPASSWORD", &password)
        .env("PGOPTIONS", "-c search_path=app,public")
        .output()
        .unwrap();
    assert_eq!(live.status.code(), Some(1));
    let mut offline = Command::cargo_bin("uroborosql-lint").unwrap();
    offline
        .current_dir(dir)
        .args(["query.sql", "--config", "config/offline.json"]);
    for (key, _) in env::vars() {
        if key.starts_with("PG") || key.starts_with("CATALOG_TEST_") {
            offline.env_remove(key);
        }
    }
    let result = offline.output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(result.stdout, live.stdout);
    assert_eq!(result.stderr, live.stderr);
    // Corruption fails acquisition and retains syntax-only diagnostics even with fail-level none.
    fs::write(dir.join("config/catalog.sqlite"), b"invalid snapshot").unwrap();
    offline
        .args(["--fail-level", "none"])
        .assert()
        .code(2)
        .stdout(contains("no-distinct"))
        .stderr(contains("snapshot"));
    // Omitted output reports one portable UTC-named file and ignores config as above.
    Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(dir)
        .args([
            "export-catalog",
            "--host",
            "127.0.0.1",
            "--port",
            &port,
            "--user",
            "catalog_reader",
            "--dbname",
            "postgres",
            "--tls-mode",
            "disable",
        ])
        .env("PGPASSWORD", &password)
        .assert()
        .success()
        .stdout("")
        .stderr(contains("Catalog exported to catalog-"));
    let names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("catalog-"))
        .collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].len(), 31);
    assert!(names[0].ends_with("Z.sqlite"));
}
