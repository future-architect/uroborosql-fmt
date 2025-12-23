mod config_store;
mod lint_config;
mod overrides;
mod types;

pub const DEFAULT_CONFIG_FILENAME: &str = ".uroborosqllintrc.json";

pub use config_store::{ConfigError, ConfigStore, ResolvedLintConfig};
pub use types::{RuleLevel, RuleSetting};
