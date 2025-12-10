use clap::Parser;
use ignore::WalkBuilder;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process,
};

use uroborosql_lint::{
    config::{
        loader::{load_config, CONFIG_FILE_NAME},
        store::ConfigStore,
        Configuration,
    },
    Diagnostic, LintError, Linter, Severity,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// SQL files or directories to lint
    #[arg(required = true)]
    paths: Vec<PathBuf>,
}

fn main() {
    let args = Args::parse();
    if let Err(err) = run(args) {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), String> {
    // 設定フェーズ: lint 対象パスからコンフィグを収集し、CWD の設定を base として選択
    let nested_configs = collect_nested_configs(&args.paths)?;
    let base_config = determine_base_config(&nested_configs);
    let store = ConfigStore::new_with_defaults(base_config, nested_configs);
    let linter = Linter::with_store(store);

    // Lint フェーズ: WalkBuilder で指定パスを再帰探索し、SQL ファイルに対して Linter を実行
    let has_lint_errors = lint_targets(&linter, args.paths)?;

    if has_lint_errors {
        Err("Linting finished with errors".into())
    } else {
        Ok(())
    }
}

fn print_diagnostic(file: &str, diagnostic: &Diagnostic) {
    let line = diagnostic.span.start.line + 1;
    let column = diagnostic.span.start.column + 1;

    let severity_str = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };

    println!(
        "{}:{}:{}: {}: [{}] {}",
        file, line, column, severity_str, diagnostic.rule_id, diagnostic.message
    );
}

// 指定されたパス配列から、各ディレクトリに存在する設定ファイルと祖先ディレクトリの設定を収集する
fn collect_nested_configs(paths: &[PathBuf]) -> Result<HashMap<PathBuf, Configuration>, String> {
    let mut nested_configs = HashMap::new();

    for path in paths {
        if !path.exists() {
            return Err(format!("{}: No such file or directory", path.display()));
        }

        scan_for_configs(path, &mut nested_configs)?;
    }

    add_ancestor_configs(paths, &mut nested_configs)?;

    Ok(nested_configs)
}

// WalkBuilder で探索し、パス配下にある .uroborosql-lintrc.json を nested_configs に登録する
fn scan_for_configs(
    path: &Path,
    nested_configs: &mut HashMap<PathBuf, Configuration>,
) -> Result<(), String> {
    let walker = WalkBuilder::new(path).hidden(false).build();

    for result in walker {
        let entry = result.map_err(|err| format!("Error walking directory: {}", err))?;

        if !entry.file_type().map_or(false, |ft| ft.is_file())
            || entry.file_name() != CONFIG_FILE_NAME
        {
            continue;
        }

        let config_path = entry.path();
        let config = load_config(config_path)
            .map_err(|e| format!("Failed to load config {}: {}", config_path.display(), e))?;

        if let Some(parent) = config_path.parent() {
            let canonical_parent =
                fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
            nested_configs.insert(canonical_parent, config);
        }
    }

    Ok(())
}

fn add_ancestor_configs(
    paths: &[PathBuf],
    nested_configs: &mut HashMap<PathBuf, Configuration>,
) -> Result<(), String> {
    for path in paths {
        if let Ok(abs_path) = fs::canonicalize(path) {
            let mut current = abs_path.parent();
            while let Some(dir) = current {
                if nested_configs.contains_key(dir) {
                    current = dir.parent();
                    continue;
                }

                let config_path = dir.join(CONFIG_FILE_NAME);
                if config_path.exists() {
                    let config = load_config(&config_path).map_err(|e| {
                        format!("Failed to load config {}: {}", config_path.display(), e)
                    })?;
                    nested_configs.insert(dir.to_path_buf(), config);
                }
                current = dir.parent();
            }
        }
    }

    Ok(())
}

fn determine_base_config(nested_configs: &HashMap<PathBuf, Configuration>) -> Configuration {
    if let Ok(cwd) = std::env::current_dir() {
        let canonical_cwd = fs::canonicalize(&cwd).unwrap_or(cwd);
        if let Some(cfg) = nested_configs.get(&canonical_cwd) {
            return cfg.clone();
        }
    }

    Configuration::default()
}

// すべての入力パスを WalkBuilder で展開し、SQL ファイルに対して lint を実行する
fn lint_targets(linter: &Linter, paths: Vec<PathBuf>) -> Result<bool, String> {
    let mut has_lint_errors = false;

    for path in paths {
        let walker = WalkBuilder::new(&path)
            .hidden(false)
            .git_ignore(true)
            .build();

        for result in walker {
            let entry = result.map_err(|err| format!("Error walking directory: {}", err))?;
            if !is_sql_file(&entry) {
                continue;
            }

            let file_path = entry.path();
            let display = file_path.display().to_string();
            let canonical_path = fs::canonicalize(file_path)
                .map_err(|err| format!("{}: failed to resolve canonical path: {}", display, err))?;

            lint_file(linter, &display, &canonical_path, &mut has_lint_errors)?;
        }
    }

    Ok(has_lint_errors)
}

fn is_sql_file(entry: &ignore::DirEntry) -> bool {
    entry.file_type().map_or(false, |ft| ft.is_file())
        && entry.path().extension().map_or(false, |ext| ext == "sql")
}

// 単一ファイルを読み込み、Linter を実行して診断を出力する（エラー有無は呼び出し元で集約）
fn lint_file(
    linter: &Linter,
    display: &str,
    canonical_path: &Path,
    has_lint_errors: &mut bool,
) -> Result<(), String> {
    let sql = fs::read_to_string(canonical_path)
        .map_err(|err| format!("{display}: failed to read file: {err}"))?;

    let diagnostics = match linter.run(canonical_path, &sql) {
        Ok(diags) => diags,
        Err(LintError::ParseError(message)) => {
            return Err(format!("{display}: failed to parse SQL: {message}"));
        }
    };

    for diagnostic in diagnostics {
        if diagnostic.severity == Severity::Error {
            *has_lint_errors = true;
        }
        print_diagnostic(display, &diagnostic);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use uroborosql_lint::config::RuleLevel;

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).unwrap();
        }
    }

    #[test]
    fn collect_nested_configs_handles_relative_paths_and_ancestors() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let subdir = root.join("nested");
        fs::create_dir(&subdir).unwrap();

        fs::write(
            root.join(CONFIG_FILE_NAME),
            r#"{"rules":{"no-distinct":"off"}}"#,
        )
        .unwrap();
        fs::write(
            subdir.join(CONFIG_FILE_NAME),
            r#"{"rules":{"no-distinct":"error"}}"#,
        )
        .unwrap();

        let sql_path = subdir.join("test.sql");
        fs::write(&sql_path, "SELECT DISTINCT * FROM table;").unwrap();

        let nested_configs =
            collect_nested_configs(&[sql_path]).expect("collect_nested_configs should succeed");

        let canonical_root = fs::canonicalize(root).unwrap();
        let canonical_subdir = fs::canonicalize(subdir).unwrap();

        let root_config = nested_configs
            .get(&canonical_root)
            .expect("root config should be present");
        let sub_config = nested_configs
            .get(&canonical_subdir)
            .expect("subdir config should be present");

        assert_eq!(
            root_config
                .rules
                .as_ref()
                .and_then(|rules| rules.get("no-distinct")),
            Some(&RuleLevel::Off)
        );
        assert_eq!(
            sub_config
                .rules
                .as_ref()
                .and_then(|rules| rules.get("no-distinct")),
            Some(&RuleLevel::Error)
        );
    }

    #[test]
    fn determine_base_config_prefers_current_directory_config() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let canonical_root = fs::canonicalize(root).unwrap();

        let mut rules = std::collections::HashMap::new();
        rules.insert("no-distinct".to_string(), RuleLevel::Error);
        let config = Configuration {
            rules: Some(rules),
            ..Default::default()
        };

        let mut nested_configs = HashMap::new();
        nested_configs.insert(canonical_root.clone(), config.clone());

        let _guard = DirGuard::change_to(root);

        let base = determine_base_config(&nested_configs);

        assert_eq!(
            base.rules.unwrap().get("no-distinct"),
            Some(&RuleLevel::Error)
        );
    }
}
