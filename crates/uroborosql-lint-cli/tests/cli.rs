use assert_cmd::Command;
use assert_fs::{fixture::ChildPath, prelude::*, TempDir};
use predicates::str::contains;

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
        .arg("init")
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
        .arg("init")
        .assert()
        .code(2)
        .stderr(contains("Config already exists"));

    config.assert("{\n  \"rules\": {}\n}\n");
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
