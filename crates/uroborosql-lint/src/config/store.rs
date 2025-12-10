use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::Configuration;
use crate::diagnostic::Severity;
use crate::linter::RuleOverride;
use crate::rule::Rule;
use crate::rules::all_rules;
use globset::Glob;
use globset::GlobSetBuilder;

use std::fmt;

pub struct ResolvedConfig<'a> {
    pub rules: Vec<(&'a dyn Rule, Severity)>,
    pub config: Arc<Configuration>,
}

pub struct ConfigStore {
    base: Arc<Configuration>,
    nested_configs: HashMap<PathBuf, Arc<Configuration>>,
    available_rules: Vec<Box<dyn Rule>>,
}

impl fmt::Debug for ConfigStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigStore")
            .field("base", &self.base)
            .field("nested_configs", &self.nested_configs)
            .field("available_rules_count", &self.available_rules.len())
            .finish()
    }
}

impl ConfigStore {
    pub fn new(
        base: Configuration,
        nested_configs: HashMap<PathBuf, Configuration>,
        rules: Vec<Box<dyn Rule>>,
    ) -> Self {
        let nested_configs = nested_configs
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();

        Self {
            base: Arc::new(base),
            nested_configs,
            available_rules: rules,
        }
    }

    // New constructor for default rules
    pub fn new_with_defaults(
        base: Configuration,
        nested_configs: HashMap<PathBuf, Configuration>,
    ) -> Self {
        Self::new(base, nested_configs, all_rules())
    }

    pub fn resolve(&self, path: &Path) -> ResolvedConfig<'_> {
        let (config, base_path) = self
            .get_nearest_config(path)
            .unwrap_or((&self.base, Path::new(".")));

        let rules = self.resolve_rules(config, path, base_path);

        ResolvedConfig {
            rules,
            config: Arc::clone(config),
        }
    }

    fn get_nearest_config<'a, 'b>(
        &'a self,
        path: &'b Path,
    ) -> Option<(&'a Arc<Configuration>, &'b Path)> {
        let mut current = path.parent();
        while let Some(dir) = current {
            if let Some(config) = self.nested_configs.get(dir) {
                return Some((config, dir));
            }
            current = dir.parent();
        }
        None
    }

    fn resolve_rules(
        &self,
        config: &Configuration,
        target_path: &Path,
        config_base: &Path,
    ) -> Vec<(&dyn Rule, Severity)> {
        let mut effective_severities: HashMap<String, Severity> = HashMap::new();

        // 1. Initialize with default severity for ALL available rules
        for rule in &self.available_rules {
            effective_severities.insert(rule.name().to_string(), rule.default_severity());
        }

        // 2. Apply global rules from config
        if let Some(rules_config) = &config.rules {
            for (rule_id, level) in rules_config {
                let override_val: RuleOverride = (*level).into();
                match override_val {
                    RuleOverride::Disabled => {
                        effective_severities.remove(rule_id);
                    }
                    RuleOverride::Enabled(severity) => {
                        if effective_severities.contains_key(rule_id) {
                            effective_severities.insert(rule_id.clone(), severity);
                        }
                    }
                }
            }
        }

        // 3. Apply overrides
        if let Some(overrides) = &config.overrides {
            for override_config in overrides {
                let mut builder = GlobSetBuilder::new();
                for pattern_str in &override_config.files {
                    if let Ok(glob) = Glob::new(pattern_str) {
                        builder.add(glob);
                    }
                }

                let glob_set = match builder.build() {
                    Ok(gs) => gs,
                    Err(_) => continue,
                };

                let relative_target = if config_base == Path::new(".") {
                    target_path
                } else {
                    match target_path.strip_prefix(config_base) {
                        Ok(p) => p,
                        Err(_) => continue,
                    }
                };

                if glob_set.is_match(relative_target) {
                    for (rule_id, level) in &override_config.rules {
                        let override_val: RuleOverride = (*level).into();
                        match override_val {
                            RuleOverride::Disabled => {
                                effective_severities.remove(rule_id);
                            }
                            RuleOverride::Enabled(severity) => {
                                if effective_severities.contains_key(rule_id) {
                                    effective_severities.insert(rule_id.clone(), severity);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 4. Match names back to rule instances
        let mut result = Vec::new();
        for rule in &self.available_rules {
            if let Some(severity) = effective_severities.get(rule.name()) {
                result.push((rule.as_ref(), *severity));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OverrideConfig;
    use crate::config::RuleLevel;

    #[test]
    fn test_resolve_base() {
        let rules_config = HashMap::from([("no-distinct".to_string(), RuleLevel::Error)]);
        let base_config = Configuration {
            rules: Some(rules_config),
            ..Default::default()
        };

        let store = ConfigStore::new_with_defaults(base_config, HashMap::new());
        let resolved = store.resolve(Path::new("src/main.sql"));

        assert!(resolved
            .rules
            .iter()
            .any(|(r, s)| r.name() == "no-distinct" && *s == Severity::Error));
    }

    #[test]
    fn test_resolve_nested() {
        let base_config = Configuration::default();

        let mut nested_rules = HashMap::new();
        nested_rules.insert("no-distinct".to_string(), RuleLevel::Error);

        let nested_config = Configuration {
            rules: Some(nested_rules),
            ..Default::default()
        };

        let mut nested_map = HashMap::new();
        let nested_path = PathBuf::from("src/subdir");
        nested_map.insert(nested_path.clone(), nested_config);

        // Ensure path exists in logic (mocking path existence not needed as we pass paths)
        let store = ConfigStore::new_with_defaults(base_config, nested_map);

        // File inside subdir should pick up nested config
        let resolved = store.resolve(Path::new("src/subdir/query.sql"));
        assert!(resolved
            .rules
            .iter()
            .any(|(r, s)| r.name() == "no-distinct" && *s == Severity::Error));
    }

    #[test]
    fn test_override_application() {
        let overrides = vec![OverrideConfig {
            files: vec!["src/*.sql".to_string()],
            rules: HashMap::from([("no-distinct".to_string(), RuleLevel::Off)]),
        }];

        let config = Configuration {
            overrides: Some(overrides),
            ..Default::default()
        };

        let store = ConfigStore::new_with_defaults(config, HashMap::new());

        // src/test.sql matches "src/*.sql" -> Rule should be Off (removed)
        let resolved = store.resolve(Path::new("src/test.sql"));
        assert!(!resolved
            .rules
            .iter()
            .any(|(r, _)| r.name() == "no-distinct"));

        // other.sql -> Rule should be present (default)
        let resolved_other = store.resolve(Path::new("other.sql"));
        assert!(resolved_other
            .rules
            .iter()
            .any(|(r, _)| r.name() == "no-distinct"));
    }
}
