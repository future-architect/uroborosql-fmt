use assert_cmd::Command;
use assert_fs::{prelude::*, NamedTempFile, TempDir};
use predicates::ord::eq;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use uroborosql_fmt::format_sql;
const DEFAULT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[test]
fn file_to_stdout() {
    // ファイル入力で標準出力へフォーマット結果を表示する

    let raw = "select 1;";
    let formatted = format_sql(raw, None, None).unwrap();

    let file = assert_fs::NamedTempFile::new("input.sql").unwrap();
    file.write_str(raw).unwrap();

    // uroborosql-fmt-cli input.sql
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg(file.path())
        .assert()
        .success()
        .stdout(eq(formatted));
}

#[test]
fn custom_config_option() {
    // 明示的に設定ファイルを指定してフォーマットする

    let raw = "select col1 from tbl;";
    let config_json = r#"{ "complement_alias": false }"#;
    let expected = format_sql(raw, Some(config_json), None).unwrap();

    // input file: input.sql
    let input_file = NamedTempFile::new("input.sql").unwrap();
    input_file.write_str(raw).unwrap();

    // config file: mycfg.json
    let config_file = NamedTempFile::new("mycfg.json").unwrap();
    config_file.write_str(config_json).unwrap();

    // uroborosql-fmt-cli --config mycfg.json input.sql
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg("--config")
        .arg(config_file.path())
        .arg(input_file.path())
        .assert()
        .success()
        .stdout(eq(expected));
}

#[test]
fn default_config_file_detected() {
    // デフォルトパスの設定ファイルが検出されフォーマットに使用される

    let raw = "select col1 from tbl;";
    let config_json = r#"{ "complement_alias": false }"#;
    let expected = format_sql(raw, Some(config_json), None).unwrap();

    // place default config file
    let dir = TempDir::new().unwrap();
    let default_config_file = dir.child(DEFAULT_CONFIG_PATH);
    default_config_file.write_str(config_json).unwrap();

    let input_file = dir.child("q.sql");
    input_file.write_str(raw).unwrap();

    // uroborosql-fmt-cli q.sql
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .current_dir(&dir)
        .arg(input_file.path())
        .assert()
        .success()
        .stdout(eq(expected));
}

#[test]
fn config_option_overrides_default() {
    // 明示的に設定ファイルを指定するとデフォルト設定のパスよりも優先される

    let raw = "select col1 from tbl;";
    let default_config_json = r#"{ "complement_alias": false }"#;
    let explicit_config_json = r#"{ "complement_alias": true }"#;
    let expected = format_sql(raw, Some(explicit_config_json), None).unwrap();

    let dir = TempDir::new().unwrap();
    // default config file
    let default_config_file = dir.child(DEFAULT_CONFIG_PATH);
    default_config_file.write_str(default_config_json).unwrap();

    // explicit config file
    let explicit_config_file = dir.child("exp.json");
    explicit_config_file
        .write_str(explicit_config_json)
        .unwrap();

    let input_file = dir.child("q.sql");
    input_file.write_str(raw).unwrap();

    // uroborosql-fmt-cli --config exp.json q.sql
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .current_dir(&dir)
        .arg("--config")
        .arg(explicit_config_file.path())
        .arg(input_file.path())
        .assert()
        .success()
        .stdout(eq(expected));
}

#[test]
fn parse_error_exit_code() {
    // パースエラーの場合、終了コード 1 (ParseError) を返す

    let invalid_sql = "SELECT FROM";

    // uroborosql-fmt-cli < echo "SELECT FROM"
    // error code 1
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .write_stdin(invalid_sql)
        .assert()
        .code(1);
}

#[test]
fn invalid_config_exit_other_error() {
    // 不正な設定ファイルの場合、終了コード 2 (OtherError) を返す

    let invalid_json = "{ invalid json }";

    let config_file = NamedTempFile::new("bad.json").unwrap();
    config_file.write_str(invalid_json).unwrap();
    let input_file = NamedTempFile::new("input.sql").unwrap();
    input_file.write_str("select 1;").unwrap();

    // uroborosql-fmt-cli --config bad.json input.sql
    // error code 2
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg("--config")
        .arg(config_file.path())
        .arg(input_file.path())
        .assert()
        .code(2);
}

#[test]
fn no_input_exit_io_error() {
    // 入力なしの場合、終了コード 2 (OtherError) を返す

    // uroborosql-fmt-cli
    // error code 2
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .assert()
        .code(2);
}

#[test]
fn nonexistent_file_exit_io_error() {
    // 存在しないファイルの場合、終了コード 3 (IoError) を返す

    // uroborosql-fmt-cli no_such_file.sql
    // error code 3
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg("no_such_file.sql")
        .assert()
        .code(3);
}

#[test]
fn write_readonly_file_exit_io_error() {
    // 読み取り専用ファイルに書き込みを試みると終了コード 3 (IoError) を返す

    let raw = "select 1;";

    let file = NamedTempFile::new("ro.sql").unwrap();
    file.write_str(raw).unwrap();

    let mut perm = fs::metadata(file.path()).unwrap().permissions();
    perm.set_mode(0o444); // read-only
    fs::set_permissions(file.path(), perm).unwrap();

    // uroborosql-fmt-cli -w ro.sql
    // error code 3
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg("-w")
        .arg(file.path())
        .assert()
        .code(3);
}

#[test]
fn write_and_check_conflict() {
    // -w と --check が競合する場合、終了コード 2 (OtherError) を返す

    // uroborosql-fmt-cli -w --check
    // error code 2
    Command::cargo_bin("uroborosql-fmt-cli")
        .unwrap()
        .arg("-w")
        .arg("--check")
        .assert()
        .failure() // clap returns non-zero; exact code 2
        .code(2);
}
