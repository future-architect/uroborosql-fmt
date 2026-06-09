use std::{env, fs, path::PathBuf};

use uroborosql_lint::{ConfigStore, Diagnostic, LintError, Linter, Severity};

use crate::args::Cli;

#[repr(i32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum ExitCode {
    IssuesFound = 1,
    ExecutionError = 2,
}

#[derive(Debug)]
pub(crate) struct CliError {
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

    pub fn print(&self) {
        if let Some(message) = &self.message {
            eprintln!("{message}");
        }
    }

    pub fn exit_code(&self) -> ExitCode {
        self.code
    }
}

pub fn run(cli: Cli) -> Result<(), CliError> {
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
