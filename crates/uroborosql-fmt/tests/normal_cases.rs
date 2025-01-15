use std::fs;
use std::path::Path;

#[derive(Debug)]
struct TestCase {
    name: String,
    sql: String,
    expected: String,
}

impl TestCase {
    fn from_files(name: &str, src_path: &Path, dst_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let sql = fs::read_to_string(src_path)?.to_string();
        let expected = fs::read_to_string(dst_path)?.to_string();

        Ok(Self {
            name: name.to_string(),
            sql,
            expected,
        })
    }
}

fn collect_test_cases() -> Vec<TestCase> {
    let src_dir = Path::new("test_normal_cases/src");
    let dst_dir = Path::new("test_normal_cases/dst");
    let mut cases = Vec::new();

    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            if src_path.extension().and_then(|s| s.to_str()) == Some("sql") {
                let file_stem = src_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                
                let dst_path = dst_dir.join(format!("{}.sql", file_stem));
                
                if dst_path.exists() {
                    match TestCase::from_files(file_stem, &src_path, &dst_path) {
                        Ok(test_case) => cases.push(test_case),
                        Err(e) => eprintln!("Error loading test case {:?}: {}", src_path, e),
                    }
                } else {
                    eprintln!("Missing dst file for test case: {:?}", src_path);
                }
            }
        }
    }

    cases
}

#[test]
fn test_normal_cases() {
    for case in collect_test_cases() {
        println!("\nTesting: {}", case.name);
        println!("Input:\n{}", case.sql);
        println!("Expected:\n{}", case.expected.escape_debug());
        
        match uroborosql_fmt::format_sql(&case.sql, None, None) {
            Ok(formatted) => {
                if formatted == case.expected {
                    println!("✅ Test passed");
                } else {
                    println!("❌ Test failed");
                    println!("Got:\n{}", formatted.escape_debug());
                    
                    panic!("Test case '{}' failed", case.name);
                }
            }
            Err(e) => {
                println!("❌ Test failed with error: {}", e);
                panic!("Test case '{}' failed with error: {}", case.name, e);
            }
        }
    }
} 