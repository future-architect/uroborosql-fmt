use std::env;
use std::fs;
use std::path::Path;

mod pgcst_util;
use pgcst_util::print_diff;

#[derive(Debug)]
struct TestCase {
    name: String,
    sql: String,
    expected: String,
}

#[derive(Debug)]
struct TestResult {
    name: String,
    status: TestStatus,
    input: String,
    expected: String,
    got: Option<String>,
    error: Option<String>,
}

#[derive(Debug)]
enum TestStatus {
    Pass,
    Fail,
    Error,
}

#[derive(Debug)]
struct TestOptions {
    fail_fast: bool,
    sort_descending: bool,
}

impl TestOptions {
    fn new() -> Self {
        TestOptions {
            fail_fast: false,
            sort_descending: false,
        }
    }

    fn from_args(args: &[String]) -> Self {
        let mut options = TestOptions::new();

        for arg in args.iter() {
            match arg.as_str() {
                "--fail-fast" => options.fail_fast = true,
                "--sort-descending" => options.sort_descending = true,
                _ => {}
            }
        }

        options
    }
}

impl TestCase {
    fn from_files(
        name: &str,
        src_path: &Path,
        dst_path: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let sql = fs::read_to_string(src_path)?.to_string();
        let expected = fs::read_to_string(dst_path)?.to_string();

        Ok(Self {
            name: name.to_string(),
            sql,
            expected,
        })
    }
}

fn print_test_report(results: &[TestResult]) {
    let total = results.len();
    let passed = results
        .iter()
        .filter(|r| matches!(r.status, TestStatus::Pass))
        .count();
    let failed = results
        .iter()
        .filter(|r| matches!(r.status, TestStatus::Fail))
        .count();
    let errors = results
        .iter()
        .filter(|r| matches!(r.status, TestStatus::Error))
        .count();

    println!("\nTest Report:");
    println!("Total test cases: {total:>4} cases");
    println!("{:<14} : {:>4} cases", "✅ Passed", passed);
    println!("{:<14} : {:>4} cases", "❌ Failed", failed);
    println!("{:<14} : {:>4} cases", "💥 Errors", errors);

    if failed > 0 || errors > 0 {
        println!("\nFailures and Errors:");
        for result in results {
            match result.status {
                TestStatus::Pass => continue,
                TestStatus::Fail => {
                    println!("\n❌ Failed: {}", result.name);
                    println!("\nInput:\n{}", result.input);
                    // println!("\nExpected:\n{}", result.expected);
                    // println!("\nGot:\n{}", result.got.as_ref().unwrap());
                    println!("\nDiff(expected vs. got):");
                    print_diff(
                        result.expected.clone(),
                        result.got.as_ref().unwrap().clone(),
                    );

                    // println!("Escaped version:");
                    // println!("sql     : {}", result.input.escape_debug());
                    // println!("expected: {}", result.expected.escape_debug());
                    // println!("got     : {}", result.got.as_ref().unwrap().escape_debug());
                }
                TestStatus::Error => {
                    println!("\n💥 Error: {}", result.name);
                    println!("\nInput:\n{}", result.input);
                    println!("\nError: {}\n", result.error.as_ref().unwrap());
                }
            }
        }
        panic!("Some tests failed");
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

                let dst_path = dst_dir.join(format!("{file_stem}.sql"));

                if dst_path.exists() {
                    match TestCase::from_files(file_stem, &src_path, &dst_path) {
                        Ok(test_case) => cases.push(test_case),
                        Err(e) => eprintln!("Error loading test case {src_path:?}: {e}"),
                    }
                } else {
                    eprintln!("Missing dst file for test case: {src_path:?}");
                }
            }
        }
    }

    cases
}

#[test]
fn test_normal_cases() {
    let args = env::args().collect::<Vec<String>>();
    let options = TestOptions::from_args(&args);
    let mut results = Vec::new();

    // sort testcases by name
    let mut cases = collect_test_cases();
    if options.sort_descending {
        cases.sort_by_key(|c| std::cmp::Reverse(c.name.clone()));
    } else {
        cases.sort_by_key(|c| c.name.clone());
    }

    for case in cases {
        println!("\nTesting: {}", case.name);

        let result = match uroborosql_fmt::format_sql(
            &case.sql,
            None,
            Some("test_normal_cases/use_new_parser.json"),
        ) {
            Ok(formatted) => {
                if formatted == case.expected {
                    println!("✅ Test passed");
                    TestResult {
                        name: case.name,
                        status: TestStatus::Pass,
                        input: case.sql,
                        expected: case.expected,
                        got: Some(formatted),
                        error: None,
                    }
                } else {
                    println!("❌ Test failed");
                    TestResult {
                        name: case.name,
                        status: TestStatus::Fail,
                        input: case.sql,
                        expected: case.expected,
                        got: Some(formatted),
                        error: None,
                    }
                }
            }
            Err(e) => {
                println!("💥 Test error");
                TestResult {
                    name: case.name,
                    status: TestStatus::Error,
                    input: case.sql,
                    expected: case.expected,
                    got: None,
                    error: Some(e.to_string()),
                }
            }
        };

        // エラー時にテストを中止するモードの場合
        if options.fail_fast {
            match &result.status {
                TestStatus::Fail | TestStatus::Error => {
                    results.push(result);
                    print_test_report(&results);
                    panic!("Test failed in fail-fast mode");
                }
                TestStatus::Pass => {
                    results.push(result);
                }
            }
        } else {
            results.push(result);
        }
    }

    print_test_report(&results);
}
