use std::{env, fs, path::PathBuf, process};

use clap::{Parser, ValueEnum};
use uroborosql_lint::{ConfigStore, Diagnostic, LintError, Linter, Severity};

#[derive(Parser, Debug)]
#[command(name = "uroborosql-lint", version, about = "SQL linter")]
struct Cli {
    /// Input SQL file
    pub input: PathBuf,

    /// Path to configuration file
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Minimum diagnostic severity that causes a non-zero exit code
    #[arg(long, value_enum, default_value_t = FailLevel::Error)]
    pub fail_level: FailLevel,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum FailLevel {
    None,
    Info,
    Warning,
    Error,
}

impl FailLevel {
    fn matches(self, severity: Severity) -> bool {
        match self {
            Self::None => false,
            Self::Info => matches!(
                severity,
                Severity::Info | Severity::Warning | Severity::Error
            ),
            Self::Warning => matches!(severity, Severity::Warning | Severity::Error),
            Self::Error => matches!(severity, Severity::Error),
        }
    }
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ExitCode {
    Ok = 0,
    IssuesFound = 1,
    ExecutionError = 2,
}

#[derive(Debug)]
struct CliError {
    code: ExitCode,
    message: Option<String>,
}

impl CliError {
    fn issues_found() -> Self {
        Self {
            code: ExitCode::IssuesFound,
            message: None,
        }
    }

    fn execution(message: impl Into<String>) -> Self {
        Self {
            code: ExitCode::ExecutionError,
            message: Some(message.into()),
        }
    }
}

fn main() {
    if let Err(err) = run() {
        if let Some(message) = err.message {
            eprintln!("{message}");
        }
        process::exit(err.code as i32);
    }

    process::exit(ExitCode::Ok as i32);
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    let linter = Linter::new();
    let cwd = env::current_dir()
        .map_err(|err| CliError::execution(format!("Failed to get cwd: {err}")))?;
    let path = resolve_input_path(cli.input, &cwd)?;
    let display = path.display().to_string();

    let sql = fs::read_to_string(&path)
        .map_err(|err| CliError::execution(format!("Failed to read {}: {}", display, err)))?;

    let config_store = ConfigStore::new(cwd, cli.config)
        .map_err(|err| CliError::execution(format!("Failed to load config: {err}")))?;

    if config_store.is_ignored(&path) {
        return Ok(());
    }

    let resolved_config = config_store.resolve(&path);

    match linter.run(&sql, &resolved_config) {
        Ok(diagnostics) => {
            let should_fail = diagnostics
                .iter()
                .any(|diagnostic| cli.fail_level.matches(diagnostic.severity));

            for diagnostic in &diagnostics {
                print_diagnostic(&display, diagnostic);
            }

            if should_fail {
                return Err(CliError::issues_found());
            }
        }
        Err(LintError::ParseError(message)) => {
            return Err(CliError::execution(format!(
                "{}: error: failed to parse SQL: {}",
                display, message
            )));
        }
    }

    Ok(())
}

fn resolve_input_path(path: PathBuf, cwd: &std::path::Path) -> Result<PathBuf, CliError> {
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };

    path.canonicalize().map_err(|err| {
        CliError::execution(format!(
            "Failed to resolve input path {}: {}",
            path.display(),
            err
        ))
    })
}

fn print_diagnostic(file: &str, diagnostic: &Diagnostic) {
    let line = diagnostic.span.start.line + 1;
    let column = diagnostic.span.start.column + 1;

    println!(
        "{file}:{line}:{column}: {severity_label}: {code}: {message}",
        severity_label = severity_label(diagnostic.severity),
        code = diagnostic.code,
        message = diagnostic.message
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

    use super::{resolve_input_path, FailLevel};
    use uroborosql_lint::Severity;

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

    #[test]
    fn fail_level_none_never_matches() {
        assert!(!FailLevel::None.matches(Severity::Info));
        assert!(!FailLevel::None.matches(Severity::Warning));
        assert!(!FailLevel::None.matches(Severity::Error));
    }

    #[test]
    fn fail_level_info_matches_all_current_severities() {
        assert!(FailLevel::Info.matches(Severity::Info));
        assert!(FailLevel::Info.matches(Severity::Warning));
        assert!(FailLevel::Info.matches(Severity::Error));
    }
}
