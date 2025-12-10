use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
    tree::collect_preorder,
};
use postgresql_cst_parser::tree_sitter;
use std::collections::HashMap;

#[derive(Debug)]
pub enum LintError {
    ParseError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOverride {
    Enabled(Severity),
    Disabled,
}

#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    overrides: HashMap<String, RuleOverride>,
}

impl From<LintOptions> for crate::config::Configuration {
    fn from(opts: LintOptions) -> Self {
        // overrides を Configuration に変換する
        // RuleOverride::Enabled(s) を RuleLevel::Error/Warn にマップする
        // 厳密には不可逆な変換になる可能性がある。
        // しかしテスト用としては通常 Error/Warn で十分。
        // 最低限の変換を実装するか、あるいは Linter::new で ConfigStore::new を使ってデフォルトにする。

        let mut rules = HashMap::new();
        for (k, v) in opts.overrides {
            match v {
                RuleOverride::Disabled => {
                    rules.insert(k, crate::config::RuleLevel::Off);
                }
                RuleOverride::Enabled(Severity::Error) => {
                    rules.insert(k, crate::config::RuleLevel::Error);
                }
                RuleOverride::Enabled(Severity::Warning) => {
                    rules.insert(k, crate::config::RuleLevel::Warn);
                }
                RuleOverride::Enabled(Severity::Info) => {
                    rules.insert(k, crate::config::RuleLevel::Warn);
                } // Info は config では Warn にマップする？ あるいは overrides にはまだ存在すべきではないかも。
            }
        }

        crate::config::Configuration {
            rules: Some(rules),
            ..Default::default()
        }
    }
}

impl LintOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_override(&self, rule_id: &str) -> Option<RuleOverride> {
        self.overrides.get(rule_id).copied()
    }

    pub fn set_override(&mut self, rule_id: impl Into<String>, override_val: RuleOverride) {
        self.overrides.insert(rule_id.into(), override_val);
    }

    pub fn with_override(mut self, rule_id: impl Into<String>, override_val: RuleOverride) -> Self {
        self.set_override(rule_id, override_val);
        self
    }
}

use crate::config::store::ConfigStore;
use crate::config::Configuration;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct Linter {
    store: Arc<ConfigStore>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        Self::with_options(LintOptions::default())
    }

    pub fn with_options(options: LintOptions) -> Self {
        let config: Configuration = options.into();
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            store: Arc::new(ConfigStore::new_with_defaults(
                config,
                base_path,
                HashMap::new(),
            )),
        }
    }

    pub fn with_store(store: ConfigStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    // 非推奨 / 互換性のため維持
    pub fn with_rules(rules: Vec<Box<dyn Rule>>) -> Self {
        let config = Configuration::default();
        // 渡されたルールを利用可能なレジストリとして使用する
        let base_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            store: Arc::new(ConfigStore::new(config, base_path, HashMap::new(), rules)),
        }
    }

    pub fn run(&self, path: &Path, sql: &str) -> Result<Vec<Diagnostic>, LintError> {
        let tree = tree_sitter::parse_2way(sql)
            .map_err(|err| LintError::ParseError(format!("{err:?}")))?;
        let root = tree.root_node();
        let nodes = collect_preorder(root.clone());
        let mut ctx = LintContext::new(sql);

        let resolved = self.store.resolve(path);

        for (rule, severity) in resolved.rules {
            rule.run_once(&root, &mut ctx, severity);

            let targets = rule.target_kinds();
            if targets.is_empty() {
                for node in &nodes {
                    rule.run_on_node(node, &mut ctx, severity);
                }
            } else {
                for node in &nodes {
                    if targets.iter().any(|kind| node.kind() == *kind) {
                        rule.run_on_node(node, &mut ctx, severity);
                    }
                }
            }
        }

        Ok(ctx.into_diagnostics())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    pub fn run_with_rules(sql: &str, rules: Vec<Box<dyn Rule>>) -> Vec<Diagnostic> {
        Linter::with_rules(rules)
            .run(Path::new("test.sql"), sql)
            .expect("lint ok")
    }

    #[test]
    fn applies_severity_override() {
        let options = LintOptions::default()
            .with_override("no-distinct", RuleOverride::Enabled(Severity::Error));
        let linter = Linter::with_options(options);
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = linter.run(Path::new("test.sql"), sql).expect("lint ok");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }
}
