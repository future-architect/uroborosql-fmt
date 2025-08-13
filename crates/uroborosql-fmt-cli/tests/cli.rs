use assert_cmd::Command;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::fs;

/// Returns a pair (raw_sql, formatted_sql)
fn sample_sql() -> (&'static str, String) {
    let raw = "select 1;";
    let formatted = uroborosql_fmt::format_sql(raw, None, None).unwrap();
    (raw, formatted)
}

#[test]
/// 標準入力からSQLを読み込んでフォーマットする
fn format_from_stdin() {
    let (raw, formatted) = sample_sql();
    let mut cmd = Command::cargo_bin("uroborosql-fmt-cli").unwrap();
    cmd.write_stdin(raw)
        .assert()
        .success()
        .stdout(predicate::eq(formatted.clone()));
}

#[test]
/// check モードと write モードは排他である
fn check_and_write_mode_conflict() {
    let mut cmd = Command::cargo_bin("uroborosql-fmt-cli").unwrap();
    cmd.arg("--check").arg("--write").assert().failure();
}

#[cfg(test)]
mod check_mode {
    use super::*;

    #[test]
    /// ファイルの内容がフォーマット結果と同じ場合は成功する
    fn check_mode_detects_no_difference() {
        let (_raw, formatted) = sample_sql();
        let file = assert_fs::NamedTempFile::new("ok.sql").unwrap();
        file.write_str(&formatted).unwrap();

        // uroborosql-fmt-cli --check ok.sql
        Command::cargo_bin("uroborosql-fmt-cli")
            .unwrap()
            .arg("--check")
            .arg(file.path())
            .assert()
            .success();
    }

    #[test]
    /// ファイルの内容がフォーマット結果と異なる場合はエラーになる
    fn check_mode_detects_difference() {
        let (raw, _formatted) = sample_sql();
        let file = assert_fs::NamedTempFile::new("ng.sql").unwrap();
        file.write_str(raw).unwrap();

        // uroborosql-fmt-cli --check ng.sql
        Command::cargo_bin("uroborosql-fmt-cli")
            .unwrap()
            .arg("--check")
            .arg(file.path())
            .assert()
            .failure();
    }
}

#[cfg(test)]
mod write_mode {
    use super::*;

    #[test]
    /// ファイルの内容をフォーマット結果で上書きする
    fn write_mode_overwrites_file() {
        let (raw, formatted) = sample_sql();
        let file = assert_fs::NamedTempFile::new("rewrite.sql").unwrap();
        file.write_str(raw).unwrap();

        // uroborosql-fmt-cli -w rewrite.sql
        Command::cargo_bin("uroborosql-fmt-cli")
            .unwrap()
            .arg("--write")
            .arg(file.path())
            .assert()
            .success();

        let content = fs::read_to_string(file.path()).unwrap();
        assert_eq!(content, formatted);
    }

    #[test]
    /// write モードでパスが指定されていなければ失敗する
    fn write_mode_fails_without_path() {
        // uroborosql-fmt-cli -w
        Command::cargo_bin("uroborosql-fmt-cli")
            .unwrap()
            .arg("--write")
            .assert()
            .failure();
    }
}
