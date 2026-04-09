use std::fs;

use tempfile::tempdir;
use uroborosql_lint::{ConfigStore, ResolvedLintConfig, Severity, DEFAULT_CONFIG_FILENAME};

fn write_config(root: &std::path::Path, contents: &str) {
    let path = root.join(DEFAULT_CONFIG_FILENAME);
    fs::write(path, contents).expect("write config");
}

fn severity_for_rule(state: &ResolvedLintConfig, name: &str) -> Option<Severity> {
    state
        .rules
        .iter()
        .find_map(|(rule, severity)| (rule.name() == name).then_some(*severity))
}

#[test]
fn reads_config_from_root_file() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "rules": {
    "no-distinct": "error"
  }
}"#,
    );

    let store = ConfigStore::new(temp.path().to_path_buf(), None).expect("config store");
    let state = store.resolve(&temp.path().join("src/query.sql"));

    assert_eq!(
        severity_for_rule(&state, "no-distinct"),
        Some(Severity::Error)
    );
}

#[test]
fn applies_overrides_by_path() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "rules": {
    "no-distinct": "error"
  },
  "overrides": [
    {
      "files": ["test/**/*.sql"],
      "rules": {
        "no-distinct": "off"
      }
    }
  ]
}"#,
    );

    let store = ConfigStore::new(temp.path().to_path_buf(), None).expect("config store");
    let state_src = store.resolve(&temp.path().join("src/main.sql"));
    let state_test = store.resolve(&temp.path().join("test/case.sql"));

    assert_eq!(
        severity_for_rule(&state_src, "no-distinct"),
        Some(Severity::Error)
    );
    assert_eq!(severity_for_rule(&state_test, "no-distinct"), None);
}

#[test]
fn ignores_paths_matching_ignore_globs() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "ignore": ["dist/**"]
}"#,
    );

    let store = ConfigStore::new(temp.path().to_path_buf(), None).expect("config store");

    assert!(store.is_ignored(&temp.path().join("dist/app.sql")));
    assert!(!store.is_ignored(&temp.path().join("src/app.sql")));
}

#[test]
fn uses_explicit_config_parent_as_root_dir() {
    let temp = tempdir().expect("tempdir");
    let config_root = temp.path().join("project");
    let cwd = temp.path().join("cwd");
    let dist_file = config_root.join("dist/app.sql");
    let src_file = config_root.join("src/app.sql");
    let test_file = config_root.join("test/case.sql");

    fs::create_dir_all(config_root.join("dist")).expect("create dist dir");
    fs::create_dir_all(config_root.join("src")).expect("create src dir");
    fs::create_dir_all(config_root.join("test")).expect("create test dir");
    fs::create_dir_all(&cwd).expect("create cwd dir");
    fs::write(&dist_file, "select 1").expect("write dist file");
    fs::write(&src_file, "select 1").expect("write src file");
    fs::write(&test_file, "select distinct foo from bar").expect("write test file");

    write_config(
        &config_root,
        r#"{
  "ignore": ["dist/**"],
  "overrides": [
    {
      "files": ["test/**/*.sql"],
      "rules": {
        "no-distinct": "off"
      }
    }
  ]
}"#,
    );

    let config_path = config_root.join(DEFAULT_CONFIG_FILENAME);
    let store = ConfigStore::new(cwd, Some(config_path)).expect("config store");

    assert_eq!(
        store.root_dir(),
        config_root
            .canonicalize()
            .expect("canonicalize config root")
    );
    assert!(store.is_ignored(&dist_file.canonicalize().expect("canonicalize dist file")));
    assert!(!store.is_ignored(&src_file.canonicalize().expect("canonicalize src file")));

    let state_test = store.resolve(&test_file.canonicalize().expect("canonicalize test file"));
    assert_eq!(severity_for_rule(&state_test, "no-distinct"), None);
}
