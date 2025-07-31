use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::Parser;

use uroborosql_fmt::error::UroboroSQLFmtError;
use uroborosql_fmt::format_sql;

/// Default configuration file name searched in current directory.
pub const DEFAULT_CONFIG_PATH: &str = ".uroborosqlfmtrc.json";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Input file. If omitted, read from STDIN.
    pub input: Option<PathBuf>,

    /// Overwrite the input file with the formatted result.
    #[arg(long, short = 'w', conflicts_with = "check")]
    pub write: bool,

    /// Check formatting; exit code 2 if the input is not properly formatted.
    #[arg(long, short = 'c', conflicts_with = "write")]
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
}

/// Run CLI processing and return `Ok(())` or an `ExitCode` on error.
pub fn run(cli: Cli) -> Result<(), ExitCode> {
    // 1) Read source SQL
    let (src, input_path) = match cli.input {
        Some(ref path) => {
            let content = fs::read_to_string(path).map_err(|_| ExitCode::OtherError)?;
            (content, Some(path.clone()))
        }
        // ファイルパスが指定されていない場合は標準入力から読み込む
        None => {
            if io::stdin().is_terminal() {
                eprintln!("error: Please provide an INPUT file or pipe SQL to STDIN.");
                return Err(ExitCode::OtherError);
            }
            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .map_err(|_| ExitCode::OtherError)?;
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
    let formatted = format_sql(&src, None, config_path).map_err(|e| match e {
        UroboroSQLFmtError::ParseError(_) => ExitCode::ParseError,
        _ => ExitCode::OtherError,
    })?;

    // 4) Option handling
    //
    // write モード:
    // - 与えられたパスのファイルをフォーマット結果で上書きする
    // - パスが指定されていなければエラー
    //
    // check モード:
    // - フォーマット結果とソースが異なる場合はエラー
    // - フォーマット結果とソースが同じ場合は成功
    //
    // モード指定なしの場合:
    // - フォーマット結果を標準出力に出力する

    if cli.write {
        let Some(path) = input_path else {
            eprintln!(
                "error: --write requires an input file path. Piping from STDIN is not supported."
            );
            return Err(ExitCode::OtherError);
        };

        if formatted != src {
            fs::write(&path, formatted).map_err(|_| ExitCode::OtherError)?;
            eprintln!("formattting done: {}", path.display());
        } else {
            eprintln!("no changes: {}", path.display());
        }
        return Ok(());
    }

    if cli.check {
        if formatted != src {
            match input_path {
                Some(ref path) => {
                    eprintln!("{}", path.display());
                    eprintln!(
                        "Code style issues found in the file. Run uroborosql-fmt-cli with --write to fix."
                    );
                }
                None => {
                    eprintln!("Code style issues found in STDIN.");
                }
            }
            return Err(ExitCode::OtherError);
        }

        println!("The input uses uroboroSQL-fmt code style.");
        return Ok(());
    }

    // 5) Output to STDOUT
    let mut stdout = io::stdout();
    stdout
        .write_all(formatted.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|_| ExitCode::OtherError)?;

    Ok(())
}
