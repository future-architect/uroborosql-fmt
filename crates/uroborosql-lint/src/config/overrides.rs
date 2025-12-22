use std::collections::HashMap;

use globset::GlobSet;

use crate::config::RuleSetting;

#[derive(Debug, Clone)]
pub struct ResolvedOverride {
    pub files: GlobSet,
    pub rules: HashMap<String, RuleSetting>,
}
