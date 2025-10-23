use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

pub struct NoUnionDistinct;

impl Rule for NoUnionDistinct {
    fn name(&self) -> &'static str {
        "no-union-distinct"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::UNION]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(range) = detect_violation_range(node) else {
            return;
        };

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Use of `UNION DISTINCT` is not recommended. (`UNION` is implicitly `UNION DISTINCT`)",
            &range,
        );
        ctx.report(diagnostic);
    }
}

fn detect_violation_range(node_union: &Node<'_>) -> Option<Range> {
    assert_eq!(node_union.kind(), SyntaxKind::UNION);

    let mut cursor = node_union.walk();
    cursor.goto_next_sibling();

    // skip comments
    while cursor.node().is_comment() {
        cursor.goto_next_sibling();
    }

    // cursor -> set_quantifier | SELECT (select_clause)
    match cursor.node().kind() {
        SyntaxKind::set_quantifier => {
            // set_quantifer has ALL or DISTINCT

            let has_all = cursor
                .node()
                .children()
                .iter()
                .any(|child| child.kind() == SyntaxKind::ALL);

            if has_all {
                // `UNION ALL` is allowed
                None
            } else {
                // `UNION DISTINCT` is NOT allowed
                Some(extend_range(node_union.range(), cursor.node().range()))
            }
        }
        SyntaxKind::SELECT => {
            // Only `UNION` pattern also means `UNION DISTINCT` implicitly
            Some(node_union.range())
        }
        _ => unreachable!(
            "CST structure error: union should be followed by set_quantifier or SELECT"
        ),
    }
}

fn extend_range(base: Range, extension: Range) -> Range {
    Range {
        start_byte: base.start_byte,
        end_byte: extension.end_byte,
        start_position: base.start_position,
        end_position: extension.end_position,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, SqlSpan};

    #[test]
    fn detects_union_without_all() {
        let sql = "SELECT 1 UNION SELECT 2;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoUnionDistinct)]);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-union-distinct")
            .expect("should detect UNION");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "UNION");
    }

    #[test]
    fn detects_union_distinct() {
        let sql = "SELECT 1 UNION DISTINCT SELECT 2;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoUnionDistinct)]);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-union-distinct")
            .expect("should detect UNION");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "UNION DISTINCT");
    }

    #[test]
    fn detects_union_distinct_with_comment() {
        let sql = "SELECT 1 UNION /* comment */ DISTINCT SELECT 2;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoUnionDistinct)]);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule_id == "no-union-distinct")
            .expect("should detect UNION DISTINCT");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "UNION /* comment */ DISTINCT");
    }

    #[test]
    fn allows_union_all() {
        let sql = "SELECT 1 UNION ALL SELECT 2;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoUnionDistinct)]);
        assert!(
            diagnostics
                .iter()
                .all(|diag| diag.rule_id != "no-union-distinct"),
            "UNION ALL should not trigger no-union-distinct rule"
        );
    }

    #[test]
    fn allows_union_all_with_comment() {
        let sql = "SELECT 1 UNION /* comment */ ALL SELECT 2;";
        let diagnostics = run_with_rules(sql, vec![Box::new(NoUnionDistinct)]);

        assert_eq!(diagnostics.len(), 0);
    }
}
