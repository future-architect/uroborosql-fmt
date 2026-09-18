use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use uroborosql_lint::Severity;

#[derive(Parser, Debug)]
#[command(
    name = "uroborosql-lint",
    version,
    about = "SQL linter",
    subcommand_negates_reqs = true,
    args_conflicts_with_subcommands = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Create a starter lint config file in the current working directory
    #[arg(long, conflicts_with_all = ["config", "fail_level", "input"])]
    pub init: bool,

    /// Input SQL file
    #[arg(required_unless_present = "init")]
    pub input: Option<PathBuf>,

    /// Path to configuration file
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Minimum diagnostic severity that causes a non-zero exit code
    #[arg(long, value_enum, default_value_t = FailLevel::Error)]
    pub fail_level: FailLevel,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum FailLevel {
    None,
    Info,
    Warning,
    Error,
}

impl FailLevel {
    pub fn matches(self, severity: Severity) -> bool {
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

#[cfg(test)]
mod tests {
    use super::FailLevel;
    use uroborosql_lint::Severity;

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

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Export the PostgreSQL catalog to a portable SQLite file (no lint config needed)
    ExportCatalog(ExportArgs),
}

#[derive(clap::Args, Debug)]
pub struct ExportArgs {
    #[arg(long)]
    pub host: String,
    #[arg(long)]
    pub user: String,
    #[arg(long)]
    pub dbname: String,
    #[arg(long, default_value_t = 5432)]
    pub port: u16,
    #[arg(long, value_enum, default_value_t = ExportTlsMode::VerifyFull)]
    pub tls_mode: ExportTlsMode,
    /// Output file; defaults to catalog-YYYYMMDDTHHMMSSZ.sqlite in UTC
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub connect_timeout_ms: Option<u64>,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub query_timeout_ms: Option<u64>,
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
    pub acquisition_timeout_ms: Option<u64>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
pub enum ExportTlsMode {
    VerifyFull,
    VerifyCa,
    Require,
    Disable,
}
