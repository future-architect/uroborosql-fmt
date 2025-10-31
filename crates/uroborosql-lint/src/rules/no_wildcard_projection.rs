use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

pub struct NoWildcardProjection;

impl Rule for NoWildcardProjection {
    fn name(&self) -> &'static str {
        "no-wildcard-projection"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::target_el]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(range) = detect_wildcard(node) else {
            return;
        };

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Wildcard projections are not allowed; list the columns explicitly.",
            &range,
        );
        ctx.report(diagnostic);
    }
}

fn detect_wildcard(node: &Node<'_>) -> Option<Range> {
    let mut cursor = node.walk();

    cursor.goto_first_child();

    match cursor.node().kind() {
        SyntaxKind::Star => Some(cursor.node().range()),
        SyntaxKind::a_expr => {
            let node = cursor.node();
            let columnref = get_columnref_from_a_expr(&node)?;

            // let indirection = get_indirection_from_columnref(&columnref)?;
            let indirection = columnref
                .children()
                .iter()
                .find(|child| child.kind() == SyntaxKind::indirection)
                .map(|child| child.clone())?;

            let last_indirection_el = indirection.children().last()?.clone();
            let star = last_indirection_el.children().last()?.clone();

            if star.kind() == SyntaxKind::Star {
                Some(last_indirection_el.range())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn get_columnref_from_a_expr<'a>(a_expr: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = a_expr.walk();
    cursor.goto_first_child();

    match cursor.node().kind() {
        SyntaxKind::c_expr => {
            cursor.goto_first_child();
            if cursor.node().kind() == SyntaxKind::columnref {
                Some(cursor.node())
            } else {
                None
            }
        }
        _ => return None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, SqlSpan};

    fn run(sql: &str) -> Vec<Diagnostic> {
        run_with_rules(sql, vec![Box::new(NoWildcardProjection)])
    }

    #[test]
    fn detects_select_star() {
        let sql = "SELECT * FROM users;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-wildcard-projection")
            .expect("should detect SELECT *");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "*");
    }

    #[test]
    fn detects_returning_star() {
        let sql = "INSERT INTO users(id) VALUES (1) RETURNING *;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-wildcard-projection")
            .expect("should detect RETURNING *");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "*");
    }

    #[test]
    fn detects_table_star() {
        let sql = "SELECT u.* FROM users u;";
        let diagnostics = run(sql);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-wildcard-projection")
            .expect("should detect .*");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], ".*");
    }

    #[test]
    fn allows_explicit_columns() {
        let sql = "SELECT id, name FROM users;";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn allows_count_star() {
        let sql = "SELECT count(*) FROM users;";
        let diagnostics = run(sql);
        assert!(diagnostics.is_empty(), "count(*) should be allowed");
    }
}
