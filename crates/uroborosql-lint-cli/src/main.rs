use clap::Parser;
use ignore::WalkBuilder;
use std::{collections::HashMap, fs, path::PathBuf, process};

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
    let mut exit_with_error = false;

    // Phase 1: Discover all configuration files
    let mut nested_configs = HashMap::new();
    let mut base_config = Configuration::default(); // Default if no root config found

    // We walk all input paths to find configs first
    for path in &args.paths {
        if !path.exists() {
            eprintln!("{}: No such file or directory", path.display());
            exit_with_error = true;
            continue;
        }

        let walker = WalkBuilder::new(path)
            .hidden(false) // Look for hidden config files
            .git_ignore(false) // Don't ignore configs in .gitignore yet? Actually configs should be checked in.
            // But we specifically want to find .uroborosql-lintrc.json
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file())
                        && entry.file_name() == CONFIG_FILE_NAME
                    {
                        let config_path = entry.path();
                        match load_config(config_path) {
                            Ok(config) => {
                                // If it's in the current directory or one of the roots, it might be the base?
                                // For now, let's treat the "highest" config found as base, or just assume CWD config is base?
                                // "Proximity Wins" doesn't strictly need a "base" vs "nested" distinction except for fallback.
                                // Let's store ALL in nested_configs keyed by directory.
                                if let Some(parent) = config_path.parent() {
                                    nested_configs.insert(parent.to_path_buf(), config);
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to load config {}: {}", config_path.display(), e);
                                // Don't exit immediately?
                            }
                        }
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }
    }

    // Determine base config:
    // If we have a config at "." or the root of the search path, use it as base.
    // ConfigStore requires a base.
    // Let's look for a config in CWD.
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(cfg) = nested_configs.get(&cwd) {
            base_config = cfg.clone();
            // Optional: Remove it from nested? No, keep it for strict directory matching.
        }
    }

    // If provided path is a file, we might have missed configs in parent directories if we only walked FROM the file.
    // We should search upwards from each argument path to find external configs too.
    for path in &args.paths {
        if let Ok(abs_path) = fs::canonicalize(path) {
            let mut current = abs_path.parent();
            while let Some(dir) = current {
                let config_path = dir.join(CONFIG_FILE_NAME);
                if !nested_configs.contains_key(dir) && config_path.exists() {
                    // Found a parent config we didn't scan
                    if let Ok(config) = load_config(&config_path) {
                        nested_configs.insert(dir.to_path_buf(), config);
                    }
                }
                current = dir.parent();
            }
        }
    }

    let store = ConfigStore::new_with_defaults(base_config, nested_configs);
    let linter = Linter::with_store(store);

    // Phase 2: Lint files
    for path in args.paths {
        let walker = WalkBuilder::new(&path)
            .hidden(false)
            .git_ignore(true) // Respect .gitignore for SQL files
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    let path = entry.path();
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        // Check extension
                        if let Some(ext) = path.extension() {
                            if ext == "sql" {
                                // Lint this file
                                let display = path.display().to_string();
                                match fs::read_to_string(path) {
                                    Ok(sql) => match linter.run(path, &sql) {
                                        Ok(diagnostics) => {
                                            for diagnostic in diagnostics {
                                                if diagnostic.severity == Severity::Error {
                                                    exit_with_error = true;
                                                }
                                                print_diagnostic(&display, &diagnostic);
                                            }
                                        }
                                        Err(LintError::ParseError(message)) => {
                                            eprintln!(
                                                "{}: failed to parse SQL: {}",
                                                display, message
                                            );
                                            exit_with_error = true;
                                        }
                                    },
                                    Err(err) => {
                                        eprintln!("{}: failed to read file: {}", display, err);
                                        exit_with_error = true;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(err) => eprintln!("Error walking directory: {}", err),
            }
        }
    }

    if exit_with_error {
        Err("Linting finished with errors".into())
    } else {
        Ok(())
    }
}

fn print_diagnostic(file: &str, diagnostic: &Diagnostic) {
    let line = diagnostic.span.start.line + 1;
    let column = diagnostic.span.start.column + 1;

    // Convert severity to string
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
