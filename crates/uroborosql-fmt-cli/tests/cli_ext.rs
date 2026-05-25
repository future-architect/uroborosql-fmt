use assert_cmd::Command;
use assert_fs::{prelude::*, NamedTempFile, TempDir};
use predicates::ord::eq;
use std::fs;

use uroborosql_fmt::format_sql;
const DEFAULT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[test]
/// モード指定なしの場合、標準出力にフォーマット結果が出力される
fn file_to_stdout() {
    let raw = "select 1;";
    let formatted = format_sql(raw, None, None).unwrap();

    let file = assert_fs::NamedTempFile::new("input.sql").unwrap();
    file.write_str(raw).unwrap();

    // uroborosql-fmt input.sql
    Command::cargo_bin("uroborosql-fmt")
        .unwrap()
        .arg(file.path())
        .assert()
        .success()
        .stdout(eq(formatted));
}

#[cfg(test)]
mod config {
    use super::*;

    #[test]
    /// 設定ファイルを指定してフォーマットする
    fn custom_config_option() {
        let raw = "select col1 from tbl;";
        let config_json = r#"{ "complement_alias": false }"#;
        let expected = format_sql(raw, Some(config_json), None).unwrap();

        // input file: input.sql
        let input_file = NamedTempFile::new("input.sql").unwrap();
        input_file.write_str(raw).unwrap();

        // config file: mycfg.json
        let config_file = NamedTempFile::new("mycfg.json").unwrap();
        config_file.write_str(config_json).unwrap();

        // uroborosql-fmt --config mycfg.json input.sql
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .arg("--config")
            .arg(config_file.path())
            .arg(input_file.path())
            .assert()
            .success()
            .stdout(eq(expected));
    }

    #[test]
    // デフォルトパスの設定ファイルが検出されフォーマットに使用される
    fn default_config_file_detected() {
        let raw = "select col1 from tbl;";
        let config_json = r#"{ "complement_alias": false }"#;
        let formatted_result_with_default_config =
            format_sql(raw, Some(config_json), None).unwrap();

        // place default config file
        let dir = TempDir::new().unwrap();
        let default_config_file = dir.child(DEFAULT_CONFIG_PATH);
        default_config_file.write_str(config_json).unwrap();

        let input_file = dir.child("q.sql");
        input_file.write_str(raw).unwrap();

        // uroborosql-fmt q.sql
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .current_dir(&dir)
            .arg(input_file.path())
            .assert()
            .success()
            .stdout(eq(formatted_result_with_default_config));
    }

    #[test]
    // 明示的に設定ファイルを指定するとデフォルト設定のパスよりも優先される
    fn config_option_overrides_default() {
        let raw = "select col1 from tbl;";
        let default_config_json = r#"{ "complement_alias": false }"#;
        let explicit_config_json = r#"{ "complement_alias": true }"#;
        let formatted_result_with_explicit_config =
            format_sql(raw, Some(explicit_config_json), None).unwrap();

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

        // uroborosql-fmt --config exp.json q.sql
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .current_dir(&dir)
            .arg("--config")
            .arg(explicit_config_file.path())
            .arg(input_file.path())
            .assert()
            .success()
            .stdout(eq(formatted_result_with_explicit_config));
    }
}

#[cfg(test)]
mod exit_code {
    use super::*;

    #[test]
    // パースエラーの場合、終了コード 1 (ParseError) を返す
    fn parse_error_exit_code() {
        let invalid_sql = "SELECT FROM";

        // uroborosql-fmt < echo "SELECT FROM"
        // error code 1
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .write_stdin(invalid_sql)
            .assert()
            .code(1);
    }

    #[test]
    /// 不正な設定ファイルの場合、終了コード 2 (OtherError) を返す
    fn invalid_config_exit_other_error() {
        let invalid_json = "{ invalid json }";

        let config_file = NamedTempFile::new("bad.json").unwrap();
        config_file.write_str(invalid_json).unwrap();
        let input_file = NamedTempFile::new("input.sql").unwrap();
        input_file.write_str("select 1;").unwrap();

        // uroborosql-fmt --config bad.json input.sql
        // error code 2
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .arg("--config")
            .arg(config_file.path())
            .arg(input_file.path())
            .assert()
            .code(2);
    }

    #[test]
    /// 入力なしの場合、終了コード 2 (OtherError) を返す
    fn no_input_exit_other_error() {
        // uroborosql-fmt
        // error code 2
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .assert()
            .code(2);
    }

    #[test]
    /// 存在しないファイルの場合、終了コード 2 (OtherError) を返す
    fn nonexistent_file_exit_other_error() {
        // uroborosql-fmt no_such_file.sql
        // error code 2
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .arg("no_such_file.sql")
            .assert()
            .code(2);
    }

    #[test]
    /// 読み取り専用ファイルに書き込みを試みると終了コード 2 (OtherError) を返す
    fn write_readonly_file_exit_other_error() {
        let raw = "select 1;";

        let file = NamedTempFile::new("ro.sql").unwrap();
        file.write_str(raw).unwrap();

        let mut perm = fs::metadata(file.path()).unwrap().permissions();
        perm.set_readonly(true);
        fs::set_permissions(file.path(), perm).unwrap();

        // uroborosql-fmt -w ro.sql
        // error code 2
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .arg("-w")
            .arg(file.path())
            .assert()
            .code(2);
    }

    #[test]
    /// -w と --check が競合する場合、終了コード 2 (OtherError) を返す
    fn write_and_check_conflict() {
        // uroborosql-fmt -w --check
        // error code 2
        Command::cargo_bin("uroborosql-fmt")
            .unwrap()
            .arg("-w")
            .arg("--check")
            .assert()
            .failure() // clap returns non-zero; exact code 2
            .code(2);
    }
}
