use std::fs;

use tempfile::tempdir;
use uroborosql_lint::{
    ConfigError, ConfigStore, ResolvedLintConfig, Severity, DEFAULT_CONFIG_FILENAME,
};

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
fn try_new_returns_none_when_default_config_is_missing() {
    let temp = tempdir().expect("tempdir");

    let store = ConfigStore::try_new(temp.path().to_path_buf(), None).expect("config store");

    assert!(store.is_none());
}

#[test]
fn try_new_returns_store_when_default_config_exists() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "rules": {
    "no-distinct": "error"
  }
}"#,
    );

    let store = ConfigStore::try_new(temp.path().to_path_buf(), None)
        .expect("config store")
        .expect("store should exist");
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
fn keeps_cwd_as_root_dir_for_explicit_config() {
    let temp = tempdir().expect("tempdir");
    let temp_root = temp.path().canonicalize().expect("canonicalize temp root");
    let config_root = temp_root.join("project");
    let cwd = temp_root.join("cwd");
    let dist_file = cwd.join("dist/app.sql");
    let src_file = cwd.join("src/app.sql");
    let test_file = cwd.join("test/case.sql");

    fs::create_dir_all(&config_root).expect("create config root");
    fs::create_dir_all(&cwd).expect("create cwd dir");
    fs::create_dir_all(cwd.join("dist")).expect("create dist dir");
    fs::create_dir_all(cwd.join("src")).expect("create src dir");
    fs::create_dir_all(cwd.join("test")).expect("create test dir");
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
    let store = ConfigStore::new(cwd.clone(), Some(config_path)).expect("config store");

    assert_eq!(store.root_dir(), cwd.as_path());
    assert!(store.is_ignored(&dist_file.canonicalize().expect("canonicalize dist file")));
    assert!(!store.is_ignored(&src_file.canonicalize().expect("canonicalize src file")));

    let state_test = store.resolve(&test_file.canonicalize().expect("canonicalize test file"));
    assert_eq!(severity_for_rule(&state_test, "no-distinct"), None);
}

#[test]
fn resolves_db_file_path_relative_to_explicit_config() {
    let temp = tempdir().expect("tempdir");
    let temp_root = temp.path().canonicalize().expect("canonicalize temp root");
    let config_root = temp_root.join("project/config");
    let cwd = temp_root.join("cwd");
    let schema_path = config_root.join("schema/schema.sql");

    fs::create_dir_all(schema_path.parent().expect("schema parent")).expect("create schema dir");
    fs::create_dir_all(&cwd).expect("create cwd dir");
    fs::write(&schema_path, "create table users (id bigint);").expect("write schema file");

    write_config(
        &config_root,
        r#"{
  "db": {
    "schemaProvider": "file",
    "path": "schema/schema.sql"
  }
}"#,
    );

    let config_path = config_root.join(DEFAULT_CONFIG_FILENAME);
    let store = ConfigStore::new(cwd, Some(config_path)).expect("config store");
    let state = store.resolve(temp_root.join("anywhere/query.sql").as_path());

    let db = state.db.expect("resolved db config");
    assert_eq!(
        format!("{db:?}"),
        format!("File {{ path: {:?} }}", schema_path)
    );
}

#[test]
fn rejects_unknown_rule_in_root_rules() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "rules": {
    "no-distnct": "error",
    "no-dstinct": "warn"
  }
}"#,
    );

    let err = ConfigStore::new(temp.path().to_path_buf(), None).expect_err("unknown rule error");

    assert!(matches!(
        err,
        ConfigError::UnknownRules { rules }
            if rules == vec!["no-distnct".to_string(), "no-dstinct".to_string()]
    ));
}

#[test]
fn rejects_unknown_rule_in_override_rules() {
    let temp = tempdir().expect("tempdir");
    write_config(
        temp.path(),
        r#"{
  "overrides": [
    {
      "files": ["test/**/*.sql"],
      "rules": {
        "no-distnct": "off"
      }
    },
    {
      "files": ["src/**/*.sql"],
      "rules": {
        "no-dstinct": "warn",
        "no-distnct": "error"
      }
    }
  ]
}"#,
    );

    let err = ConfigStore::new(temp.path().to_path_buf(), None).expect_err("unknown rule error");

    assert!(matches!(
        err,
        ConfigError::UnknownRules { rules }
            if rules == vec!["no-distnct".to_string(), "no-dstinct".to_string()]
    ));
}
