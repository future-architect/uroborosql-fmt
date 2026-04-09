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

    fs::create_dir_all(config_root.join("dist")).expect("create dist dir");
    fs::create_dir_all(config_root.join("src")).expect("create src dir");
    fs::create_dir_all(&cwd).expect("create cwd dir");

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

    assert_eq!(store.root_dir(), config_root.as_path());
    assert!(store.is_ignored(&config_root.join("dist/app.sql")));
    assert!(!store.is_ignored(&config_root.join("src/app.sql")));

    let state_test = store.resolve(&config_root.join("test/case.sql"));
    assert_eq!(severity_for_rule(&state_test, "no-distinct"), None);
}
