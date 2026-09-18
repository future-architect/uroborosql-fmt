use assert_cmd::Command;
use assert_fs::{fixture::ChildPath, prelude::*, TempDir};
use predicates::{prelude::PredicateBooleanExt, str::contains};

fn write_sql(temp: &TempDir, name: &str, sql: &str) -> ChildPath {
    let file = temp.child(name);
    file.write_str(sql).expect("write sql");
    file
}

#[test]
fn missing_config_returns_execution_error() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT DISTINCT id FROM users;");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg(input.path())
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("No lint config found"));
}

#[test]
fn explicit_config_emits_warning_by_default() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT DISTINCT id FROM users;");
    let config = temp.child(".uroborosqllintrc.json");
    config.write_str("{}").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--fail-level")
        .arg("error")
        .arg(input.path())
        .assert()
        .code(0)
        .stdout(contains("warning: no-distinct"));
}

#[test]
fn warning_fails_with_fail_level_warning() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT DISTINCT id FROM users;");
    let config = temp.child(".uroborosqllintrc.json");
    config.write_str("{}").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--fail-level")
        .arg("warning")
        .arg(input.path())
        .assert()
        .code(1)
        .stdout(contains("warning: no-distinct"));
}

#[test]
fn error_fails_by_default() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT DISTINCT id FROM users;");
    let config = temp.child(".uroborosqllintrc.json");
    config
        .write_str(
            r#"{
  "rules": {
    "no-distinct": "error"
  }
}"#,
        )
        .expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg(input.path())
        .assert()
        .code(1)
        .stdout(contains("error: no-distinct"));
}

#[test]
fn invalid_directive_warning_does_not_fail_by_default() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(
        &temp,
        "query.sql",
        "-- uroborosql-lint-disable definitely-not-a-rule\nSELECT 1;\n",
    );
    let config = temp.child(".uroborosqllintrc.json");
    config.write_str("{}").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg(input.path())
        .assert()
        .code(0)
        .stdout(contains("warning: invalid-lint-directive"));
}

#[test]
fn invalid_directive_warning_fails_with_fail_level_warning() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(
        &temp,
        "query.sql",
        "-- uroborosql-lint-disable definitely-not-a-rule\nSELECT 1;\n",
    );
    let config = temp.child(".uroborosqllintrc.json");
    config.write_str("{}").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--fail-level")
        .arg("warning")
        .arg(input.path())
        .assert()
        .code(1)
        .stdout(contains("warning: invalid-lint-directive"));
}

#[test]
fn parse_failure_returns_missing_config_error_without_config() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT FROM");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg(input.path())
        .assert()
        .code(2)
        .stdout("")
        .stderr(contains("No lint config found"));
}

#[test]
fn init_creates_default_config_file() {
    let temp = TempDir::new().expect("tempdir");
    let config = temp.child(".uroborosqllintrc.json");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--init")
        .assert()
        .code(0)
        .stdout(contains("Created"));

    config.assert("{}\n");
}

#[test]
fn init_does_not_overwrite_existing_file() {
    let temp = TempDir::new().expect("tempdir");
    let config = temp.child(".uroborosqllintrc.json");
    config
        .write_str("{\n  \"rules\": {}\n}\n")
        .expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--init")
        .assert()
        .code(2)
        .stderr(contains("Config already exists"));

    config.assert("{\n  \"rules\": {}\n}\n");
}

#[test]
fn init_conflicts_with_input() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT 1;");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--init")
        .arg(input.path())
        .assert()
        .code(2)
        .stderr(contains("cannot be used with"));
}

#[test]
fn parse_failure_returns_execution_error_when_config_is_present() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT FROM");
    let config = temp.child(".uroborosqllintrc.json");
    config.write_str("{}").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg(input.path())
        .assert()
        .code(2)
        .stderr(contains("failed to parse SQL"));
}

#[test]
fn invalid_config_returns_execution_error() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT 1;");
    let config = temp.child("bad.json");
    config.write_str("{ invalid json }").expect("write config");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--config")
        .arg(config.path())
        .arg(input.path())
        .assert()
        .code(2)
        .stderr(contains("Failed to load config"));
}

#[test]
fn invalid_fail_level_returns_usage_error() {
    let temp = TempDir::new().expect("tempdir");
    let input = write_sql(&temp, "query.sql", "SELECT 1;");

    Command::cargo_bin("uroborosql-lint")
        .expect("cargo_bin")
        .current_dir(temp.path())
        .arg("--fail-level")
        .arg("bogus")
        .arg(input.path())
        .assert()
        .code(2)
        .stderr(contains("invalid value 'bogus'"));
}

#[test]
fn catalog_skip_and_exclusion_are_visible_without_connecting() {
    let temp = TempDir::new().unwrap();
    let input = write_sql(&temp, "query.sql", "SELECT DISTINCT id FROM users;");
    let cfg = temp.child("lint.json");
    for (json, expected) in [
        (r#"{}"#, "skipped (not configured)"),
        (
            r#"{"db":{"schemaProvider":"server","host":"","user":"","dbname":""},"rules":{"no-unknown-reference":"off"}}"#,
            "skipped (rule disabled)",
        ),
        (
            r#"{"db":{"schemaProvider":"server","host":"","user":"","dbname":""}}"#,
            "complete=0 excluded=1 failed=0; unsupported syntax",
        ),
    ] {
        cfg.write_str(json).unwrap();
        Command::cargo_bin("uroborosql-lint")
            .unwrap()
            .current_dir(temp.path())
            .arg(input.path())
            .arg("--config")
            .arg(cfg.path())
            .assert()
            .success()
            .stdout(contains("no-distinct"))
            .stderr(contains(expected));
    }
}

#[test]
fn unavailable_file_catalog_retains_diagnostics_and_overrides_fail_none() {
    let temp = TempDir::new().unwrap();
    let input = write_sql(
        &temp,
        "query.sql",
        "SELECT DISTINCT id FROM users; SELECT missing FROM users;",
    );
    let cfg = temp.child("lint.json");
    cfg.write_str(r#"{"db":{"schemaProvider":"file","path":"absent.sqlite"}}"#)
        .unwrap();
    Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(temp.path())
        .arg(input.path())
        .arg("--config")
        .arg(cfg.path())
        .args(["--fail-level", "none"])
        .assert()
        .code(2)
        .stdout(contains("no-distinct"))
        .stderr(contains("complete=0 excluded=1 failed=1").and(contains(
            if cfg!(feature = "sqlite-catalog") {
                "Catalog snapshot could not be opened"
            } else {
                "File catalog is unavailable"
            },
        )));
    assert!(!temp.path().join("absent.sqlite").exists());
}

#[test]
fn recovered_source_reports_deferred_checks_without_accessing_file() {
    let temp = TempDir::new().unwrap();
    let input = write_sql(&temp, "query.sql", "SELECT x.id FROM /*#table*/ AS x;");
    let cfg = temp.child("lint.json");
    cfg.write_str(r#"{"db":{"schemaProvider":"file","path":"absent.sqlite"}}"#)
        .unwrap();
    Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(temp.path())
        .arg(input.path())
        .arg("--config")
        .arg(cfg.path())
        .assert()
        .success()
        .stderr(contains(
            "complete=1 excluded=0 failed=0; recovered source checks deferred=1",
        ));
    assert!(!temp.path().join("absent.sqlite").exists());
}

#[test]
fn invalid_connection_keeps_cst_and_reports_safe_classified_failure_once() {
    let temp = TempDir::new().unwrap();
    let input = write_sql(
        &temp,
        "query.sql",
        "SELECT DISTINCT id FROM users; SELECT id FROM users; SELECT id FROM users;",
    );
    let cfg = temp.child("lint.json");
    cfg.write_str(r#"{"db":{"schemaProvider":"server","host":"private-host,other","user":"private-user","password":"private-password","dbname":"app"}}"#).unwrap();
    let output = Command::cargo_bin("uroborosql-lint")
        .unwrap()
        .current_dir(temp.path())
        .arg(input.path())
        .arg("--config")
        .arg(cfg.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("no-distinct"));
    let status = String::from_utf8(output.stderr).unwrap();
    assert!(status.contains("failed=2"));
    let reason = if cfg!(feature = "postgres-catalog") {
        "Invalid catalog host"
    } else {
        "PostgreSQL catalog is unavailable"
    };
    assert_eq!(status.matches(reason).count(), 1);
    assert!(!status.contains("private-"));
}
