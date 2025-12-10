use std::fs;
use tempfile::tempdir;
use uroborosql_lint::linter::RuleOverride;
use uroborosql_lint::{
    config::loader::{find_config_file, is_ignored, load_config, resolve_lint_options},
    Severity,
};

#[test]
fn test_config_discovery_and_resolution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // 1. Setup Filesystem
    // root/
    //   .uroborosql-lintrc.json (Global Config)
    //   src/
    //     main.sql
    //     ignored.sql
    //   subdir/
    //     .uroborosql-lintrc.json (Subdir Config)
    //     sub.sql

    let global_config = r#"{
        "rules": {
            "global-rule": "warn"
        },
        "overrides": [
            {
                "files": ["src/*.sql"],
                "rules": {
                    "global-rule": "error"
                }
            }
        ],
        "ignore": ["src/ignored.sql"]
    }"#;

    let subdir_config = r#"{
        "rules": {
            "subdir-rule": "error"
        }
    }"#;

    fs::write(root.join(".uroborosql-lintrc.json"), global_config).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(root.join("src/main.sql"), "").unwrap();
    fs::write(root.join("src/ignored.sql"), "").unwrap();

    fs::create_dir(root.join("subdir")).unwrap();
    fs::write(root.join("subdir/.uroborosql-lintrc.json"), subdir_config).unwrap();
    fs::write(root.join("subdir/sub.sql"), "").unwrap();

    // 2. Test Discovery
    let src_path = root.join("src");
    let config_path = find_config_file(&src_path).expect("Should find global config");
    assert_eq!(config_path, root.join(".uroborosql-lintrc.json"));

    let subdir_path = root.join("subdir");
    let sub_config_path = find_config_file(&subdir_path).expect("Should find subdir config");
    assert_eq!(sub_config_path, root.join("subdir/.uroborosql-lintrc.json"));

    // 3. Test Loading & Resolution (Global)
    let config = load_config(&config_path).expect("Should load config");
    let config_dir = config_path.parent().unwrap();

    {
        // src/main.sql (Overridden to Error)
        let target = dir.path().join("src/main.sql");
        let options = resolve_lint_options(&config, &target, &config_dir);
        // "src/*.sql" has "global-rule": "error"
        assert_eq!(
            options.get_override("global-rule"),
            Some(RuleOverride::Enabled(Severity::Error))
        );
    }

    {
        let target = dir.path().join("src/ignored.sql");
        let options = resolve_lint_options(&config, &target, &config_dir);
        // Ignored file is checked separately by `is_ignored`, resolve logic generally applies rules anyway if called,
        // but here we are checking overrides.
        // Wait, does resolve_lint_options check ignore? No, `is_ignored` does.
        // The test might have been checking something else?
        // "ignore" field logic is tested via `is_ignored`.
        // Let's check overrides matching.
        // The config has "overrides" for "src/*.sql".
        // "ignored.sql" matches "src/*.sql" glob? Yes.
        assert_eq!(
            options.get_override("global-rule"),
            Some(RuleOverride::Enabled(Severity::Error))
        );
    }

    // src/ignored.sql (Ignored)
    assert!(is_ignored(
        &config,
        &root.join("src/ignored.sql"),
        config_dir
    ));
    assert!(!is_ignored(&config, &root.join("src/main.sql"), config_dir));

    // 4. Test Loading & Resolution (Subdir - Proximity Wins)
    let sub_config = load_config(&sub_config_path).expect("Should load sub config");
    let sub_config_dir = sub_config_path.parent().unwrap();

    // subdir/sub.sql
    let opts_sub = resolve_lint_options(&sub_config, &root.join("subdir/sub.sql"), sub_config_dir);

    // Should have subdir-rule
    assert_eq!(
        opts_sub.get_override("subdir-rule"),
        Some(RuleOverride::Enabled(Severity::Error))
    );
    // Should NOT have global-rule (No cascading)
    assert_eq!(opts_sub.get_override("global-rule"), None);
}
