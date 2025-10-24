use std::{env, fs, path::PathBuf, process};

use uroborosql_lint::{Diagnostic, LintError, Linter};

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let Some(path_os) = args.next() else {
        return Err(format!(
            "Usage: {} <SQL_FILE>...",
            env::args()
                .next()
                .unwrap_or_else(|| "uroborosql-lint-cli".to_string())
        ));
    };

    if args.next().is_some() {
        return Err("Only a single SQL file can be specified".into());
    }

    let linter = Linter::new();
    let mut exit_with_error = false;

    let path = PathBuf::from(path_os);
    let display = path.display().to_string();

    let sql =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read {}: {}", display, err))?;

    match linter.run(&sql) {
        Ok(diagnostics) => {
            for diagnostic in diagnostics {
                print_diagnostic(&display, &diagnostic);
            }
        }
        Err(LintError::ParseError(message)) => {
            eprintln!("{}: failed to parse SQL: {}", display, message);
            exit_with_error = true;
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

    println!(
        "{}:{}:{}: {}: {}",
        file, line, column, diagnostic.rule_id, diagnostic.message
    );
}
