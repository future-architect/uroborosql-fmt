use std::{env, fs, path::PathBuf, process};

use clap::Parser;
use uroborosql_lint::{ConfigStore, Diagnostic, LintError, Linter, Severity};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Input SQL file
    pub input: PathBuf,

    /// Path to configuration file
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    let linter = Linter::new();
    let mut exit_with_error = false;

    let cwd = env::current_dir().map_err(|err| format!("Failed to get cwd: {err}"))?;
    let path = resolve_input_path(cli.input, &cwd)?;
    let display = path.display().to_string();

    let sql =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read {}: {}", display, err))?;

    let config_store =
        ConfigStore::new(cwd, cli.config).map_err(|err| format!("Failed to load config: {err}"))?;

    if config_store.is_ignored(&path) {
        return Ok(());
    }

    let resolved_config = config_store.resolve(&path);

    match linter.run(&sql, &resolved_config) {
        Ok(diagnostics) => {
            for diagnostic in diagnostics {
                print_diagnostic(&display, &diagnostic);
            }
        }
        Err(LintError::ParseError(message)) => {
            eprintln!("{}: error: failed to parse SQL: {}", display, message);
            exit_with_error = true;
        }
    }

    if exit_with_error {
        Err("Linting finished with errors".into())
    } else {
        Ok(())
    }
}

fn resolve_input_path(path: PathBuf, cwd: &std::path::Path) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };

    path.canonicalize()
        .map_err(|err| format!("Failed to resolve input path {}: {}", path.display(), err))
}

fn print_diagnostic(file: &str, diagnostic: &Diagnostic) {
    let line = diagnostic.span.start.line + 1;
    let column = diagnostic.span.start.column + 1;

    println!(
        "{}:{}:{}: {}: {}: {}",
        file,
        line,
        column,
        severity_label(diagnostic.severity),
        diagnostic.rule_id,
        diagnostic.message
    );
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::resolve_input_path;

    #[test]
    fn resolves_relative_input_path_from_cwd() {
        let temp = tempdir().expect("tempdir");
        let cwd = temp.path().join("runner");
        let project = temp.path().join("project");
        let input = project.join("query.sql");

        fs::create_dir_all(&cwd).expect("create cwd");
        fs::create_dir_all(&project).expect("create project");
        fs::write(&input, "select 1").expect("write input");

        let resolved = resolve_input_path("../project/query.sql".into(), &cwd).expect("resolve");

        assert_eq!(resolved, input.canonicalize().expect("canonicalize input"));
    }

    #[test]
    fn keeps_absolute_input_path() {
        let temp = tempdir().expect("tempdir");
        let input = temp.path().join("query.sql");

        fs::write(&input, "select 1").expect("write input");

        let resolved =
            resolve_input_path(input.clone(), temp.path()).expect("resolve absolute input");

        assert_eq!(resolved, input.canonicalize().expect("canonicalize input"));
    }
}
