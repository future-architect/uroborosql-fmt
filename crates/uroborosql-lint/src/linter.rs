use crate::{
    context::LintContext, diagnostic::Diagnostic, rule::Rule, rules::NoDistinct,
    tree::collect_preorder,
};
use postgresql_cst_parser::tree_sitter;

#[derive(Debug)]
pub enum LintError {
    ParseError(String),
}

pub struct Linter {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        let rules: Vec<Box<dyn Rule>> = vec![Box::new(NoDistinct)];
        Self { rules }
    }

    pub fn run(&self, sql: &str) -> Result<Vec<Diagnostic>, LintError> {
        let tree =
            tree_sitter::parse(sql).map_err(|err| LintError::ParseError(format!("{err:?}")))?;
        let root = tree.root_node();
        let nodes = collect_preorder(root.clone());
        let mut ctx = LintContext::new(sql);

        for rule in &self.rules {
            rule.run_once(&root, &mut ctx);

            let targets = rule.target_kinds();
            if targets.is_empty() {
                for node in &nodes {
                    rule.run_on_node(node, &mut ctx);
                }
            } else {
                for node in &nodes {
                    if targets.iter().any(|kind| node.kind() == *kind) {
                        rule.run_on_node(node, &mut ctx);
                    }
                }
            }
        }

        Ok(ctx.into_diagnostics())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn detects_distinct_keyword() {
        let linter = Linter::new();
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = linter.run(sql).expect("lint ok");
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule_id, "no-distinct");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(sql[diagnostic.span.start.byte..diagnostic.span.end.byte]
            .eq_ignore_ascii_case("distinct"));
    }

    #[test]
    fn no_diagnostics_without_distinct() {
        let linter = Linter::new();
        let sql = "SELECT id FROM users;";
        let diagnostics = linter.run(sql).expect("lint ok");
        assert!(diagnostics.is_empty());
    }
}
