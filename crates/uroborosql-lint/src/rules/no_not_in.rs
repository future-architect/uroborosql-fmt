use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

pub struct NoNotIn;

impl Rule for NoNotIn {
    fn name(&self) -> &'static str {
        "no-not-in"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::in_expr]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(range) = detect_not_in(node) else {
            return;
        };

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Avoid using NOT IN; prefer NOT EXISTS or other alternatives to handle NULL correctly.",
            &range,
        );
        ctx.report(diagnostic);
    }
}

fn detect_not_in(node: &Node<'_>) -> Option<Range> {
    // detect `NOT_LA IN_P in_expr` sequense

    let in_expr_node = node;
    if in_expr_node.kind() != SyntaxKind::in_expr {
        return None;
    }

    // IN_P
    let in_node = prev_node_skipping_comments(in_expr_node)?;
    if in_node.kind() != SyntaxKind::IN_P {
        return None;
    }

    // NOT_LA
    let not_node = prev_node_skipping_comments(&in_node)?;
    if not_node.kind() != SyntaxKind::NOT_LA {
        return None;
    }

    Some(not_node.range().extended_by(&in_expr_node.range()))
}

fn prev_node_skipping_comments<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut current = node.clone();
    loop {
        let prev = current.prev_sibling()?;
        if !prev.is_comment() {
            return Some(prev);
        }
        current = prev;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, SqlSpan};

    #[test]
    fn detects_simple_not_in() {
        let sql = "SELECT value FROM users WHERE id NOT IN (1, 2);";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoNotIn)]);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-not-in")
            .expect("expected NOT IN to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "NOT IN (1, 2)");
    }

    #[test]
    fn detects_not_in_with_comment() {
        let sql = "SELECT value FROM users WHERE id NOT /* comment */ IN (1);";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoNotIn)]);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-not-in")
            .expect("expected NOT IN to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "NOT /* comment */ IN (1)");
    }

    #[test]
    fn detects_not_in_with_subquery() {
        let sql = "SELECT value FROM users WHERE id NOT IN (SELECT id FROM admins);";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoNotIn)]);

        assert!(
            diagnostics.iter().any(|diag| diag.rule_id == "no-not-in"),
            "expected NOT IN subquery to be detected"
        );
    }

    #[test]
    fn allows_in_without_not() {
        let sql = "SELECT value FROM users WHERE id IN (1, 2);";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoNotIn)]);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_not_between() {
        let sql = "SELECT value FROM users WHERE id NOT BETWEEN 1 AND 5;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoNotIn)]);
        assert!(diagnostics.is_empty(), "NOT BETWEEN should be allowed");
    }
}
