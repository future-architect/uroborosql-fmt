use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use uroborosql_lint::Severity;

#[derive(Parser, Debug)]
#[command(
    name = "uroborosql-lint",
    version,
    about = "SQL linter",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub lint: LintArgs,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a starter lint config file in the current working directory
    Init,
}

#[derive(Args, Debug)]
pub struct LintArgs {
    /// Input SQL file
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
