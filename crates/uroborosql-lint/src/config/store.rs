use std::borrow::Cow;
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
    base_path: PathBuf,
    nested_configs: HashMap<PathBuf, Arc<Configuration>>,
    available_rules: Vec<Box<dyn Rule>>,
}

impl fmt::Debug for ConfigStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigStore")
            .field("base", &self.base)
            .field("base_path", &self.base_path)
            .field("nested_configs", &self.nested_configs)
            .field("available_rules_count", &self.available_rules.len())
            .finish()
    }
}

impl ConfigStore {
    pub fn new(
        base: Configuration,
        base_path: PathBuf,
        nested_configs: HashMap<PathBuf, Configuration>,
        rules: Vec<Box<dyn Rule>>,
    ) -> Self {
        let nested_configs = nested_configs
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();

        Self {
            base: Arc::new(base),
            base_path,
            nested_configs,
            available_rules: rules,
        }
    }

    pub fn new_with_defaults(
        base: Configuration,
        base_path: PathBuf,
        nested_configs: HashMap<PathBuf, Configuration>,
    ) -> Self {
        Self::new(base, base_path, nested_configs, all_rules())
    }

    pub fn resolve(&self, path: &Path) -> ResolvedConfig<'_> {
        let (config, base_path) = self
            .get_nearest_config(path)
            .map(|(cfg, cfg_base)| (cfg, Cow::Owned(cfg_base)))
            .unwrap_or_else(|| (&self.base, Cow::Borrowed(self.base_path.as_path())));

        let rules = self.resolve_rules(config, path, &base_path);

        ResolvedConfig {
            rules,
            config: Arc::clone(config),
        }
    }

    fn get_nearest_config<'a>(&'a self, path: &Path) -> Option<(&'a Arc<Configuration>, PathBuf)> {
        let mut current = path.parent();
        while let Some(dir) = current {
            if let Some(config) = self.nested_configs.get(dir) {
                return Some((config, dir.to_path_buf()));
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

        // 1. 利用可能なすべてのルールについて、デフォルトの重要度で初期化する
        for rule in &self.available_rules {
            effective_severities.insert(rule.name().to_string(), rule.default_severity());
        }

        // 2. 設定からグローバルルールを適用する
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

        // 3. overrides を適用する
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

                let relative_target = match target_path.strip_prefix(config_base) {
                    Ok(p) => p,
                    Err(_) => continue,
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

        // 4. 名前をルールインスタンスに戻す
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

        let base_path = PathBuf::from("/project");
        let store = ConfigStore::new_with_defaults(base_config, base_path.clone(), HashMap::new());
        let resolved = store.resolve(Path::new("/project/src/main.sql"));

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

        let base_path = PathBuf::from("/project");
        let mut nested_map = HashMap::new();
        let nested_path = base_path.join("src/subdir");
        nested_map.insert(nested_path.clone(), nested_config);

        // パスがロジック内に存在することを確認します（パスを渡すため、モックされたパスの存在は必要ありません）
        let store = ConfigStore::new_with_defaults(base_config, base_path, nested_map);

        // サブディレクトリ内のファイルはネストされた設定を取得する必要があります
        let resolved = store.resolve(Path::new("/project/src/subdir/query.sql"));
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

        let base_path = PathBuf::from("/project");
        let store = ConfigStore::new_with_defaults(config, base_path, HashMap::new());

        // src/test.sql は "src/*.sql" に一致 -> ルールは Off (削除) になるべき
        let resolved = store.resolve(Path::new("/project/src/test.sql"));
        assert!(!resolved
            .rules
            .iter()
            .any(|(r, _)| r.name() == "no-distinct"));

        // other.sql -> ルールは存在するべき (デフォルト)
        let resolved_other = store.resolve(Path::new("/project/other.sql"));
        assert!(resolved_other
            .rules
            .iter()
            .any(|(r, _)| r.name() == "no-distinct"));
    }
}
