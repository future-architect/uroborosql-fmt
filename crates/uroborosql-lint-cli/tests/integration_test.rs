use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cli_directory_recursion() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // 1. Setup Root Config (disable no-distinct globally)
    let root_config = r#"{
        "rules": {
            "no-distinct": "off"
        }
    }"#;
    fs::write(root.join(".uroborosql-lintrc.json"), root_config).unwrap();

    // 2. Setup Subdir Config (enable no-distinct)
    let subdir = root.join("subdir");
    fs::create_dir(&subdir).unwrap();
    let subdir_config = r#"{
        "rules": {
            "no-distinct": "error"
        }
    }"#;
    fs::write(subdir.join(".uroborosql-lintrc.json"), subdir_config).unwrap();

    // 3. Create SQL files
    // distinct.sql in root -> should PASS (rule off)
    let root_sql = "SELECT DISTINCT * FROM my_table;";
    fs::write(root.join("root.sql"), root_sql).unwrap();

    // distinct.sql in subdir -> should FAIL (rule error)
    fs::write(subdir.join("sub.sql"), root_sql).unwrap();

    // 4. Run CLI
    let status = Command::new(env!("CARGO_BIN_EXE_uroborosql-lint-cli"))
        .arg(root)
        .output()
        .expect("failed to execute process");

    let stdout = String::from_utf8_lossy(&status.stdout);
    let stderr = String::from_utf8_lossy(&status.stderr);

    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    // 5. Assertions
    // We expect failure because sub.sql fails due to no-distinct error
    assert!(!status.status.success());

    // Check root.sql output
    // Should NOT have [no-distinct]
    let root_lines: Vec<&str> = stdout.lines().filter(|l| l.contains("root.sql")).collect();
    let root_has_distinct = root_lines.iter().any(|l| l.contains("[no-distinct]"));
    assert!(
        !root_has_distinct,
        "root.sql matched no-distinct but should be disabled. Output:\n{}",
        stdout
    );

    // Should have [no-wildcard-projection] (just to verify linting ran)
    let root_has_wildcard = root_lines
        .iter()
        .any(|l| l.contains("[no-wildcard-projection]"));
    assert!(
        root_has_wildcard,
        "root.sql should match no-wildcard-projection"
    );

    // Check sub.sql output
    // Should have [no-distinct]
    let sub_lines: Vec<&str> = stdout.lines().filter(|l| l.contains("sub.sql")).collect();
    let sub_has_distinct = sub_lines.iter().any(|l| l.contains("[no-distinct]"));
    assert!(sub_has_distinct, "sub.sql should match no-distinct");
}
