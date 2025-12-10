pub mod loader;
pub mod store;
pub mod structure;

pub use structure::*;

use crate::diagnostic::Severity;
use crate::linter::RuleOverride;

impl From<RuleLevel> for RuleOverride {
    fn from(level: RuleLevel) -> Self {
        match level {
            RuleLevel::Error => RuleOverride::Enabled(Severity::Error),
            RuleLevel::Warn => RuleOverride::Enabled(Severity::Warning),
            RuleLevel::Off => RuleOverride::Disabled,
        }
    }
}
