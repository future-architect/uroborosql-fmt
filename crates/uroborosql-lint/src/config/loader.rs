use super::Configuration;
use crate::linter::LintOptions;
use globset::{Glob, GlobSetBuilder};
use ignore::gitignore::GitignoreBuilder;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE_NAME: &str = ".uroborosql-lintrc.json";

/// Find the configuration file starting from the given directory and traversing up.
pub fn find_config_file(start_dir: &Path) -> Option<PathBuf> {
    let mut current = start_dir;
    loop {
        let config_path = current.join(CONFIG_FILE_NAME);
        if config_path.exists() {
            return Some(config_path);
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    None
}

/// Load configuration from the specified path.
pub fn load_config(path: &Path) -> Result<Configuration, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let config: Configuration = serde_json::from_str(&content)?;
    Ok(config)
}

/// Resolve effective LintOptions for a specific file based on the configuration.
pub fn resolve_lint_options(
    config: &Configuration,
    target_path: &Path,
    config_base: &Path,
) -> LintOptions {
    let mut options = LintOptions::new();

    // 1. Apply global rules
    if let Some(rules) = &config.rules {
        for (rule_id, level) in rules {
            options.set_override(rule_id.clone(), (*level).into());
        }
    }

    // 2. Apply overrides
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
                    options.set_override(rule_id.clone(), (*level).into());
                }
            }
        }
    }

    options
}

/// Check if a file is ignored by the configuration.
pub fn is_ignored(config: &Configuration, target_path: &Path, config_base: &Path) -> bool {
    if let Some(ignore_patterns) = &config.ignore {
        let mut builder = GitignoreBuilder::new(config_base);
        for pattern in ignore_patterns {
            let _ = builder.add_line(None, pattern);
        }

        // build() returns Gitignore
        let ignore_matcher = match builder.build() {
            Ok(m) => m,
            Err(_) => return false,
        };

        // matched_path_or_any_parents returns Match which can be Ignore or Allow or None
        // We want to know if it's ignored.
        match ignore_matcher.matched_path_or_any_parents(target_path, false) {
            ignore::Match::Ignore(_) => return true,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuleLevel;
    use crate::diagnostic::Severity;
    use crate::linter::RuleOverride;
    use std::collections::HashMap;

    #[test]
    fn test_find_not_found() {
        let path = Path::new("/non-existent/path");
        assert_eq!(find_config_file(path), None);
    }

    #[test]
    fn test_resolve_options_overrides() {
        let mut rules = HashMap::new();
        rules.insert("rule-1".to_string(), RuleLevel::Warn);

        let mut override_rules = HashMap::new();
        override_rules.insert("rule-1".to_string(), RuleLevel::Off);

        let overrides = vec![crate::config::OverrideConfig {
            files: vec!["test/**/*.sql".to_string()],
            rules: override_rules,
        }];

        let config = Configuration {
            rules: Some(rules),
            overrides: Some(overrides),
            ..Default::default()
        };

        let root = Path::new("/app");

        // Normal file -> Should be Warn
        let opts = resolve_lint_options(&config, Path::new("/app/src/main.sql"), root);
        assert_eq!(
            opts.get_override("rule-1"),
            Some(RuleOverride::Enabled(Severity::Warning))
        );

        // Test file -> Should be Disabled (Off)
        let opts = resolve_lint_options(&config, Path::new("/app/test/foo.sql"), root);
        assert_eq!(opts.get_override("rule-1"), Some(RuleOverride::Disabled));
    }

    #[test]
    fn test_ignore_check() {
        let config = Configuration {
            ignore: Some(vec!["dist".to_string(), "target".to_string()]),
            ..Default::default()
        };
        let root = Path::new("/app");

        // ignore match is robust. "dist" matches "dist" dir and children.
        // But we are passing paths. ignoring /app/dist/bundle.js

        // GitignoreBuilder expects paths relative to root or absolute?
        // We constructed it with `config_base` (/app).
        // It should handle absolute paths if checking under that root.

        assert!(is_ignored(&config, Path::new("/app/dist/bundle.js"), root));
        assert!(is_ignored(&config, Path::new("/app/dist"), root));
        assert!(!is_ignored(&config, Path::new("/app/src/main.js"), root));
    }
}
