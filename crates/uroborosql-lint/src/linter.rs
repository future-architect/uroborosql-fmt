use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
    rules::{
        MissingTwoWaySample, NoDistinct, NoFunctionOnColumnInJoinOrWhere, NoNotIn, NoUnionDistinct,
        NoWildcardProjection, TooLargeInList,
    },
    tree::collect_preorder,
};
use postgresql_cst_parser::tree_sitter;
use std::collections::HashMap;

#[derive(Debug)]
pub enum LintError {
    ParseError(String),
}

#[derive(Debug, Clone, Default)]
pub struct LintOptions {
    severity_overrides: HashMap<String, Severity>,
}

impl LintOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn severity_for(&self, rule_id: &str) -> Option<Severity> {
        self.severity_overrides.get(rule_id).copied()
    }

    pub fn set_severity_override(&mut self, rule_id: impl Into<String>, severity: Severity) {
        self.severity_overrides.insert(rule_id.into(), severity);
    }

    pub fn with_severity_override(
        mut self,
        rule_id: impl Into<String>,
        severity: Severity,
    ) -> Self {
        self.set_severity_override(rule_id, severity);
        self
    }
}

pub struct Linter {
    rules: Vec<Box<dyn Rule>>,
    options: LintOptions,
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
        Self::with_rules_and_options(default_rules(), options)
    }

    pub fn with_rules(rules: Vec<Box<dyn Rule>>) -> Self {
        Self::with_rules_and_options(rules, LintOptions::default())
    }

    pub fn with_rules_and_options(rules: Vec<Box<dyn Rule>>, options: LintOptions) -> Self {
        Self { rules, options }
    }

    pub fn run(&self, sql: &str) -> Result<Vec<Diagnostic>, LintError> {
        let tree = tree_sitter::parse_2way(sql)
            .map_err(|err| LintError::ParseError(format!("{err:?}")))?;
        let root = tree.root_node();
        let nodes = collect_preorder(root.clone());
        let mut ctx = LintContext::new(sql);

        for rule in &self.rules {
            let severity = self
                .options
                .severity_for(rule.name())
                .unwrap_or_else(|| rule.default_severity());

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

fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoDistinct),
        Box::new(NoNotIn),
        Box::new(NoUnionDistinct),
        Box::new(NoFunctionOnColumnInJoinOrWhere),
        Box::new(NoWildcardProjection),
        Box::new(MissingTwoWaySample),
        Box::new(TooLargeInList),
    ]
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    pub fn run_with_rules(sql: &str, rules: Vec<Box<dyn Rule>>) -> Vec<Diagnostic> {
        Linter::with_rules(rules).run(sql).expect("lint ok")
    }

    #[test]
    fn applies_severity_override() {
        let options = LintOptions::default().with_severity_override("no-distinct", Severity::Error);
        let linter = Linter::with_options(options);
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = linter.run(sql).expect("lint ok");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }
}
