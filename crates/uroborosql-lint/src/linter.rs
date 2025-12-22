use crate::{
    context::LintContext,
    diagnostic::Diagnostic,
    rule::Rule,
    rules::{
        MissingTwoWaySample, NoDistinct, NoFunctionOnColumnInJoinOrWhere, NoNotIn, NoUnionDistinct,
        NoWildcardProjection, TooLargeInList,
    },
    tree::collect_preorder,
    ResolvedLintConfig,
};
use postgresql_cst_parser::tree_sitter;

#[derive(Debug)]
pub enum LintError {
    ParseError(String),
}

#[derive(Debug, Default)]
pub struct Linter;

impl Linter {
    pub fn new() -> Self {
        Self
    }

    pub fn run(
        &self,
        sql: &str,
        resolved_config: &ResolvedLintConfig,
    ) -> Result<Vec<Diagnostic>, LintError> {
        let tree = tree_sitter::parse_2way(sql)
            .map_err(|err| LintError::ParseError(format!("{err:?}")))?;
        let root = tree.root_node();
        let nodes = collect_preorder(root.clone());
        let mut ctx = LintContext::new(sql);

        for (rule, severity) in &resolved_config.rules {
            rule.run_once(&root, &mut ctx, *severity);

            let targets = rule.target_kinds();
            if targets.is_empty() {
                for node in &nodes {
                    rule.run_on_node(node, &mut ctx, *severity);
                }
            } else {
                for node in &nodes {
                    if targets.iter().any(|kind| node.kind() == *kind) {
                        rule.run_on_node(node, &mut ctx, *severity);
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
    use crate::{
        diagnostic::Severity,
        rules::{NoDistinct, RuleEnum},
    };

    fn state_from_rules(rules: Vec<(RuleEnum, Severity)>) -> ResolvedLintConfig {
        ResolvedLintConfig { rules, db: None }
    }

    pub fn run_with_rules(sql: &str, rules: Vec<RuleEnum>) -> Vec<Diagnostic> {
        let resolved_rules = rules
            .into_iter()
            .map(|rule| {
                let severity = rule.default_severity();
                (rule, severity)
            })
            .collect();
        let state = state_from_rules(resolved_rules);

        Linter::new().run(sql, &state).expect("lint ok")
    }

    #[test]
    fn applies_severity_override() {
        let state = state_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Error)]);
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = Linter::new().run(sql, &state).expect("lint ok");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }
}
