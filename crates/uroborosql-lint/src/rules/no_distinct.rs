use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};

pub struct NoDistinct;

impl Rule for NoDistinct {
    fn name(&self) -> &'static str {
        "no-distinct"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::DISTINCT]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "DISTINCT is not recommended.",
            &node.range(),
        );
        ctx.report(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diagnostic::Severity, linter::tests::run_with_rules, rule::Rule};

    #[test]
    fn detects_distinct_keyword() {
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoDistinct) as Box<dyn Rule>]);
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule_id, "no-distinct");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(sql[diagnostic.span.start.byte..diagnostic.span.end.byte]
            .eq_ignore_ascii_case("distinct"));
    }

    #[test]
    fn ignores_select_without_distinct() {
        let sql = "SELECT id FROM users;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoDistinct) as Box<dyn Rule>]);
        assert!(diagnostics.is_empty());
    }
}
