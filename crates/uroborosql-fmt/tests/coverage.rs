#[derive(Debug)]
enum TestStatus {
    Supported,   // 新パーサーで対応済み
    Unsupported, // まだ未対応
    Skipped,     // 意図的にスキップ
}

#[derive(Debug)]
struct TestResult {
    file_path: String,
    status: TestStatus,
    error_message: Option<String>,
}

#[derive(Debug)]
struct TestReportConfig {
    show_summary: bool,
    show_by_category: bool,
    show_supported_cases: bool,
    show_skipped_cases: bool,
    show_failed_cases: bool,
}

impl Default for TestReportConfig {
    fn default() -> Self {
        Self {
            show_summary: true,
            show_by_category: true,
            show_supported_cases: true,
            show_skipped_cases: false,
            show_failed_cases: false,
        }
    }
}

fn collect_test_files(dir: &str) -> Vec<String> {
    use std::fs;
    let mut files = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_test_files(path.to_str().unwrap()));
            } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                if let Some(path_str) = path.to_str() {
                    files.push(path_str.to_string());
                }
            }
        }
    }
    files
}

fn extract_category(file_path: &str) -> String {
    let path = std::path::Path::new(file_path);
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn try_format_with_new_parser(file_path: &str) -> Result<String, String> {
    use std::fs;

    // 2way-sqlのケースは明示的にスキップ
    if file_path.contains("2way_sql") {
        return Err("2way-sql is intentionally skipped".to_string());
    }

    let content =
        fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;

    // TODO: 新しいパーサーでの処理を実装
    // とりあえず全てUnsupportedとして扱う
    Err("❌ Not implemented yet".to_string())
}

fn print_coverage_report(results: &[TestResult], config: &TestReportConfig) {
    use std::collections::HashMap;

    if config.show_summary {
        let total = results.len();
        let supported = results
            .iter()
            .filter(|r| matches!(r.status, TestStatus::Supported))
            .count();
        let skipped = results
            .iter()
            .filter(|r| matches!(r.status, TestStatus::Skipped))
            .count();
        let unsupported = total - supported - skipped;

        println!("\nCoverage Report:");
        println!("Total test cases: {:>4} cases", total);
        println!(
            "{:<14} : {:>4} cases, {:>6.1}%",
            "✅ Supported",
            supported,
            (supported as f64 / total as f64) * 100.0
        );
        println!(
            "{:<15} : {:>4} cases, {:>6.1}%",
            "⏭️ Skipped",
            skipped,
            (skipped as f64 / total as f64) * 100.0
        );
        println!(
            "{:<14} : {:>4} cases, {:>6.1}%",
            "❌ Unsupported",
            unsupported,
            (unsupported as f64 / total as f64) * 100.0
        );
    }

    if config.show_by_category {
        // カテゴリ別の集計
        let mut by_category: HashMap<String, Vec<&TestResult>> = HashMap::new();
        for result in results {
            let category = extract_category(&result.file_path);
            by_category.entry(category).or_default().push(result);
        }

        // 最長のカテゴリ名の長さを取得
        let max_category_len = by_category.keys().map(|k| k.len()).max().unwrap_or(0);

        // カテゴリをソートして出力
        println!("\nBy Category:");
        let mut categories: Vec<_> = by_category.keys().collect();
        categories.sort();

        for category in categories {
            let cases = &by_category[category];
            let supported = cases
                .iter()
                .filter(|r| matches!(r.status, TestStatus::Supported))
                .count();
            let skipped = cases
                .iter()
                .filter(|r| matches!(r.status, TestStatus::Skipped))
                .count();
            let total = cases.len();

            println!(
                "  {:<width$}: {:>3}/{:<3} ({:>6.1}%) [Skipped: {:>3}]",
                category,
                supported,
                total,
                (supported as f64 / total as f64) * 100.0,
                skipped,
                width = max_category_len
            );
        }
    }

    if config.show_supported_cases {
        println!("\nSupported Cases:");
        for result in results {
            if matches!(result.status, TestStatus::Supported) {
                println!("✅ {}", result.file_path);
            }
        }
    }

    if config.show_skipped_cases {
        println!("\nSkipped Cases:");
        for result in results {
            if matches!(result.status, TestStatus::Skipped) {
                println!(
                    "{} - {}",
                    result.file_path,
                    result
                        .error_message
                        .as_ref()
                        .unwrap_or(&"No message".to_string())
                );
            }
        }
    }

    if config.show_failed_cases {
        println!("\nFailed Cases:");
        for result in results {
            if matches!(result.status, TestStatus::Unsupported) {
                if let Some(error) = &result.error_message {
                    println!("{} - {}", result.file_path, error);
                }
            }
        }
    }
}

fn run_test_suite() -> Vec<TestResult> {
    let mut results = Vec::new();

    for test_file in collect_test_files("testfiles/src") {
        let result = match try_format_with_new_parser(&test_file) {
            Ok(_) => TestResult {
                file_path: test_file,
                status: TestStatus::Supported,
                error_message: None,
            },
            Err(e) if e.contains("intentionally skipped") => TestResult {
                file_path: test_file,
                status: TestStatus::Skipped,
                error_message: Some(e),
            },
            Err(e) => TestResult {
                file_path: test_file,
                status: TestStatus::Unsupported,
                error_message: Some(e),
            },
        };
        results.push(result);
    }
    results
}

#[test]
fn test_with_coverage() {
    let results = run_test_suite();

    print_coverage_report(&results, &TestReportConfig::default());
}
