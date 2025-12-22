use std::{env, fs, path::PathBuf, process};

use clap::Parser;
use uroborosql_lint::{ConfigStore, Diagnostic, LintError, Linter};

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

    let path = cli.input;
    let display = path.display().to_string();

    let sql =
        fs::read_to_string(&path).map_err(|err| format!("Failed to read {}: {}", display, err))?;

    let cwd = env::current_dir().map_err(|err| format!("Failed to get cwd: {err}"))?;
    let config_store =
        ConfigStore::new(cwd, cli.config).map_err(|err| format!("Failed to load config: {err}"))?;

    if config_store.is_ignored(&path) {
        return Ok(());
    }

    let state = config_store.resolve(&path);

    match linter.run(&sql, &state) {
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
