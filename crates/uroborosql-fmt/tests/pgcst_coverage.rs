mod pgcst_util;
use pgcst_util::print_diff;

#[derive(Debug)]
enum TestStatus {
    Supported,           // 新パーサーで対応済み
    Unsupported(String), // 未対応（理由を保持）
    Skipped,             // 意図的にスキップ
}

#[derive(Debug)]
struct TestResult {
    file_path: String,
    status: TestStatus,
}

#[derive(Debug)]
struct TestReportConfig {
    show_summary: bool,
    show_by_category: bool,
    show_supported_cases: bool,
    show_skipped_cases: bool,
    show_failed_cases: bool,
    show_error_annotations: bool,
}

impl Default for TestReportConfig {
    fn default() -> Self {
        Self {
            show_summary: true,
            show_by_category: true,
            show_supported_cases: true,
            show_skipped_cases: false,
            show_failed_cases: false,
            show_error_annotations: false,
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
    use uroborosql_fmt::error::UroboroSQLFmtError;

    // 2way-sqlのケースは明示的にスキップ
    if file_path.contains("2way_sql") {
        return Err("2way-sql is intentionally skipped".to_string());
    }

    let input = fs::read_to_string(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
    let dst_path = file_path.replace("/src", "/dst");
    let expected =
        fs::read_to_string(&dst_path).map_err(|e| format!("Failed to read dst file: {}", e))?;

    match uroborosql_fmt::format_sql(&input, None, Some("test_normal_cases/use_new_parser.json")) {
        Ok(formatted) => {
            if formatted.trim() == expected.trim() {
                Ok(formatted)
            } else {
                println!("\n❌ {}", file_path);
                println!("Diff(expected vs. got):");
                print_diff(expected.trim(), formatted.trim());
                Err(format!("Formatting result does not match"))
            }
        }
        Err(e) => {
            // エラーの種類に応じてメッセージを詳細化
            let error_detail = match e {
                UroboroSQLFmtError::IllegalOperation(msg) => format!("Illegal operation: {}", msg),
                UroboroSQLFmtError::UnexpectedSyntax(msg) => format!("Syntax error: {}", msg),
                UroboroSQLFmtError::Unimplemented(msg) => format!("Unimplemented: {}", msg),
                UroboroSQLFmtError::FileNotFound(msg) => format!("File not found: {}", msg),
                UroboroSQLFmtError::IllegalSettingFile(msg) => format!("Invalid config: {}", msg),
                UroboroSQLFmtError::Rendering(msg) => format!("Rendering error: {}", msg),
                UroboroSQLFmtError::Runtime(msg) => format!("Runtime error: {}", msg),
                UroboroSQLFmtError::Validation { error_msg, .. } => {
                    format!("Validation error: {}", error_msg)
                }
            };
            Err(format!("❌ {}", error_detail))
        }
    }
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
                println!("  {} - intentionally skipped", result.file_path);
            }
        }
    }

    if config.show_failed_cases {
        println!("\nFailed Cases (by error type):");

        // エラーの種類でグループ化して出力
        let mut syntax_errors = Vec::new();
        let mut validation_errors = Vec::new();
        let mut unimplemented_errors = Vec::new();
        let mut config_errors = Vec::new();
        let mut runtime_errors = Vec::new();
        let mut other_errors = Vec::new();

        for result in results {
            if let TestStatus::Unsupported(error_msg) = &result.status {
                let (message, _annotation) = if let Some(idx) = error_msg.find('\n') {
                    (error_msg[..idx].to_string(), Some(&error_msg[idx..]))
                } else {
                    (error_msg.clone(), None)
                };

                if error_msg.contains("Syntax error:") {
                    syntax_errors.push((result.file_path.clone(), message, _annotation));
                } else if error_msg.contains("Validation error:") {
                    validation_errors.push((result.file_path.clone(), message, _annotation));
                } else if error_msg.contains("Unimplemented:") {
                    unimplemented_errors.push((result.file_path.clone(), message, _annotation));
                } else if error_msg.contains("Invalid config:") {
                    config_errors.push((result.file_path.clone(), message, _annotation));
                } else if error_msg.contains("Runtime error:") {
                    runtime_errors.push((result.file_path.clone(), message, _annotation));
                } else {
                    other_errors.push((result.file_path.clone(), message, _annotation));
                }
            }
        }

        // エラータイプごとに出力
        print_error_group(
            "Syntax Errors",
            &syntax_errors,
            config.show_error_annotations,
        );
        print_error_group(
            "Validation Errors",
            &validation_errors,
            config.show_error_annotations,
        );
        print_error_group(
            "Unimplemented Features",
            &unimplemented_errors,
            config.show_error_annotations,
        );
        print_error_group(
            "Configuration Errors",
            &config_errors,
            config.show_error_annotations,
        );
        print_error_group(
            "Runtime Errors",
            &runtime_errors,
            config.show_error_annotations,
        );
        print_error_group("Other Errors", &other_errors, config.show_error_annotations);
    }
}

fn print_error_group(
    group_name: &str,
    errors: &[(String, String, Option<&str>)],
    show_annotations: bool,
) {
    if !errors.is_empty() {
        println!("\n{}:", group_name);
        for (file, message, annotation) in errors {
            println!("  {} - {}", file, message);
            if show_annotations {
                if let Some(ann) = annotation {
                    println!("{}", ann);
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
            },
            Err(e) if e.contains("intentionally skipped") => TestResult {
                file_path: test_file,
                status: TestStatus::Skipped,
            },
            Err(e) => TestResult {
                file_path: test_file,
                status: TestStatus::Unsupported(e),
            },
        };
        results.push(result);
    }
    results
}

#[test]
fn test_with_coverage() {
    let results = run_test_suite();

    let config = TestReportConfig::default();

    // let mut config = TestReportConfig::default();
    // config.show_failed_cases = true; // 失敗したケースを表示
    // config.show_error_annotations = true; // アノテーションを表示

    print_coverage_report(&results, &config);
}
