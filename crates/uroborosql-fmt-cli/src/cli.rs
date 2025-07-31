use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::Parser;

use uroborosql_fmt::format_sql;

/// Default configuration file name searched in current directory.
pub const DEFAULT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Input file. If omitted, read from STDIN.
    pub input: Option<PathBuf>,

    /// Overwrite the input file with the formatted result.
    #[arg(long, short = 'w', conflicts_with = "check")]
    pub write: bool,

    /// Check formatting; exit code 4 if the input is not properly formatted.
    #[arg(long, conflicts_with = "write")]
    pub check: bool,

    #[arg(long, value_name = "FILE", default_value = DEFAULT_CONFIG_PATH, help = "Path to configuration file.")]
    pub config: PathBuf,
}

#[repr(i32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExitCode {
    Ok = 0,
    ParseError = 1,
    OtherError = 2,
    IoError = 3,
    Diff = 4,
}

/// Run CLI processing and return `Ok(())` or an `ExitCode` on error.
pub fn run(cli: Cli) -> Result<(), ExitCode> {
    // 1) Read source SQL
    let (src, input_path) = match cli.input {
        Some(ref path) => {
            let content = fs::read_to_string(path).map_err(|_| ExitCode::IoError)?;
            (content, Some(path.clone()))
        }
        None => {
            if io::stdin().is_terminal() {
                eprintln!("error: Please provide an INPUT file or pipe SQL to STDIN.");
                return Err(ExitCode::IoError);
            }
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|_| ExitCode::IoError)?;
            (buf, None)
        }
    };

    // 2) Resolve config path
    let config_path: Option<&str> = if cli.config.exists() {
        cli.config.to_str()
    } else {
        None
    };

    // 3) Format SQL
    let formatted = match format_sql(&src, None, config_path) {
        Ok(res) => res,
        Err(e) => {
            return match e {
                uroborosql_fmt::error::UroboroSQLFmtError::ParseError(_) => {
                    Err(ExitCode::ParseError)
                }
                _ => Err(ExitCode::OtherError),
            };
        }
    };

    // 4) Option handling
    if cli.write {
        let path = input_path.ok_or(ExitCode::IoError)?;
        if formatted != src {
            fs::write(&path, formatted).map_err(|_| ExitCode::IoError)?;
        }
        return Ok(());
    }

    if cli.check {
        if formatted != src {
            return Err(ExitCode::Diff);
        }
        return Ok(());
    }

    // 5) Output to STDOUT
    let mut stdout = io::stdout();
    stdout
        .write_all(formatted.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|_| ExitCode::IoError)?;

    Ok(())
}
