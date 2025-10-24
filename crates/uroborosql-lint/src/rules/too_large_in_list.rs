use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

const MAX_IN_ELEMENTS: usize = 100;

pub struct TooLargeInList;

impl Rule for TooLargeInList {
    fn name(&self) -> &'static str {
        "too-large-in-list"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::in_expr]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(count) = count_expr_list_elements(node) else {
            return;
        };

        if count <= MAX_IN_ELEMENTS {
            return;
        }

        let message = format!("IN list has {count} elements (limit: {MAX_IN_ELEMENTS})");

        let diagnostic = Diagnostic::new(self.name(), severity, message, &node.range());
        ctx.report(diagnostic);
    }
}

fn count_expr_list_elements(node: &Node<'_>) -> Option<usize> {
    // find expr_list among children
    let expr_list = node
        .children()
        .into_iter()
        .find(|child| child.kind() == SyntaxKind::expr_list)?;

    let count = expr_list
        .children()
        .into_iter()
        .filter(|child| !child.is_comment())
        .filter(|child| child.kind() == SyntaxKind::a_expr)
        .count();

    Some(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linter::tests::run_with_rules;

    fn run(sql: &str) -> Vec<Diagnostic> {
        run_with_rules(sql, vec![Box::new(TooLargeInList)])
    }

    fn repetitions(count: usize) -> String {
        (0..count)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[test]
    fn allows_at_threshold() {
        let sql = format!(
            "SELECT * FROM users WHERE id IN ({});",
            repetitions(MAX_IN_ELEMENTS)
        );
        let diagnostics = run(&sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn warns_over_threshold() {
        let sql = format!(
            "SELECT * FROM users WHERE id IN ({});",
            repetitions(MAX_IN_ELEMENTS + 1)
        );
        let diagnostics = run(&sql);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "too-large-in-list");
    }

    #[test]
    fn ignores_subquery() {
        let sql = "SELECT * FROM users WHERE id IN (SELECT id FROM admins);";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn handles_comments_inside_list() {
        let sql = format!(
            "SELECT * FROM users WHERE id IN (1, /*a*/ 2, 3 {});",
            (4..=(MAX_IN_ELEMENTS + 1))
                .map(|i| format!(", /*c*/ {}", i))
                .collect::<String>()
        );
        let diagnostics = run(&sql);
        assert_eq!(diagnostics.len(), 1);
    }
}
