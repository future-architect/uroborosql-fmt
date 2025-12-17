use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use crate::{config::ConfigurationObject, rule::RuleEnum, Severity};

pub struct ConfigStore {
    /// The base configuration
    base: ConfigurationObject,
    nested_configs: BTreeMap<PathBuf, ConfigurationObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub rules: Vec<(RuleEnum, Severity)>,
    // env or context might be needed
}

impl ConfigStore {
    pub fn new(base: ConfigurationObject) -> Self {
        Self {
            base,
            nested_configs: BTreeMap::new(),
        }
    }

    pub fn add_nested_config(&mut self, path: PathBuf, config: ConfigurationObject) {
        self.nested_configs.insert(path, config);
    }

    /// Resolve the configuration for the given path.
    pub fn resolve(&self, path: &Path) -> ResolvedConfig {
        unimplemented!()
    }
}
