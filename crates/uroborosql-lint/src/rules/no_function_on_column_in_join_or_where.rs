use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

/// Detects function usage on columns in JOIN or WHERE conditions.
/// source: https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#:~:text=1-,%E3%82%A4%E3%83%B3%E3%83%87%E3%83%83%E3%82%AF%E3%82%B9%E3%82%AB%E3%83%A9%E3%83%A0%E3%81%AB%E9%96%A2%E6%95%B0,-%E3%82%92%E9%80%9A%E3%81%97%E3%81%9F
pub struct NoFunctionOnColumnInJoinOrWhere;

impl Rule for NoFunctionOnColumnInJoinOrWhere {
    fn name(&self) -> &'static str {
        "no-function-on-column-in-join-or-where"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::func_expr]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        assert_eq!(node.kind(), SyntaxKind::func_expr);

        let Some(range) = detect_wrapping_column(node) else {
            return;
        };

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Functions in JOIN or WHERE conditions can prevent index usage; rewrite without wrapping the column.",
            &range,
        );
        ctx.report(diagnostic);
    }
}

fn detect_wrapping_column(func_expr: &Node) -> Option<Range> {
    if !is_in_detection_range(func_expr) {
        return None;
    }

    // Exclude over clause and filter clause
    let function_body = func_expr
        .first_child()
        .expect("func_expr must have a first child");

    // Check if there is a direct column reference
    if !contains_column_for_current_func_expr(&function_body) {
        return None;
    }

    // Exclude sibling clauses like OVER() or FILTER() from the diagnostic range
    Some(function_body.range())
}

fn is_in_detection_range(func_expr: &Node) -> bool {
    // Traverse upward; if `join_qual` or `where_clause` is found, it's in the detection range
    // If `select_no_parens` is encountered before reaching `join_qual` or `where_clause`, it's outside the detection range

    let mut node = func_expr.parent();
    while let Some(current) = node {
        match current.kind() {
            SyntaxKind::join_qual | SyntaxKind::where_clause => return true,
            SyntaxKind::select_no_parens => return false,
            _ => (),
        }
        node = current.parent();
    }

    false
}

fn contains_column_for_current_func_expr(function_body: &Node) -> bool {
    assert!(
        matches!(
            function_body.kind(),
            SyntaxKind::func_application | SyntaxKind::json_aggregate_func | SyntaxKind::func_expr_common_subexpr
        ),
        "function_body should be func_application, json_aggregate_func, or func_expr_common_subexpr"
    );

    let func_expr = function_body
        .parent()
        .expect("function_body should always have func_expr as its parent");

    // Returns true if the current `func_expr` directly wraps any column reference.
    function_body
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::columnref)
        .any(|column_ref| {
            // Traverse upward from columnref and check if the first encountered func_expr is the current function.
            let mut parent = column_ref.parent();
            while let Some(ancestor) = parent {
                if ancestor.kind() == SyntaxKind::func_expr {
                    return ancestor == func_expr;
                }
                parent = ancestor.parent();
            }
            false
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, Diagnostic, SqlSpan};

    fn run(sql: &str) -> Vec<Diagnostic> {
        run_with_rules(sql, vec![Box::new(NoFunctionOnColumnInJoinOrWhere)])
    }

    mod where_clause {
        use super::*;

        #[test]
        fn allows_plain_column_comparisons() {
            let sql = "SELECT * FROM users WHERE users.id = 1;";
            let diagnostics = run(sql);

            assert!(diagnostics.is_empty(),);
        }

        #[test]
        fn allows_function_with_constant() {
            let sql =
                "SELECT * FROM users WHERE users.created_at >= to_date('20160101', 'YYYYMMDD');";
            let diagnostics = run(sql);

            assert!(diagnostics.is_empty());
        }

        #[test]
        fn detects_function_in_where_clause() {
            let sql = "SELECT * FROM users WHERE lower(users.name) = 'foo';";
            let diagnostics = run(sql);

            assert!(diagnostics
                .iter()
                .any(|diag| diag.rule_id == "no-function-on-column-in-join-or-where"),);

            assert_eq!(diagnostics.len(), 1);

            let SqlSpan { start, end } = diagnostics[0].span;
            assert_eq!(&sql[start.byte..end.byte], "lower(users.name)");
        }

        #[test]
        fn detects_coalesce_usage() {
            let sql = "SELECT * FROM users WHERE coalesce(users.deleted_at, users.updated_at) IS NOT NULL;";
            let diagnostics = run(sql);

            assert_eq!(diagnostics.len(), 1);

            let SqlSpan { start, end } = diagnostics[0].span;
            assert_eq!(
                &sql[start.byte..end.byte],
                "coalesce(users.deleted_at, users.updated_at)"
            );
        }

        #[test]
        fn detects_function_with_mixed_arguments() {
            let sql = "SELECT * FROM users WHERE coalesce(users.deleted_at, trim(users.name)) IS NOT NULL;";
            let diagnostics = run(sql);

            assert_eq!(
                diagnostics.len(),
                2,
                "coalesce and trim should both be reported when they wrap columns"
            );

            let spans: Vec<_> = diagnostics
                .iter()
                .map(|diag| &sql[diag.span.start.byte..diag.span.end.byte])
                .collect();

            assert!(
                spans.iter().any(|span| span == &"trim(users.name)"),
                "trim should be reported"
            );

            assert!(
                spans
                    .iter()
                    .any(|span| span == &"coalesce(users.deleted_at, trim(users.name))"),
                "outer coalesce should still be reported"
            );
        }

        #[test]
        fn detects_only_innermost_function_in_nested_chain() {
            let sql = "SELECT * FROM users WHERE lower(trim(users.email)) = 'foo';";
            let diagnostics = run(sql);

            assert_eq!(
                diagnostics.len(),
                1,
                "only the innermost function wrapping the column should be flagged"
            );

            let SqlSpan { start, end } = diagnostics[0].span;
            assert_eq!(&sql[start.byte..end.byte], "trim(users.email)");
        }

        #[test]
        fn allows_function_on_other_branch_without_column_reference() {
            let sql =
                "SELECT * FROM users u JOIN vendors v ON u.id = v.user_id WHERE (u.name = v.name) OR 'const' = lower('CONST');";
            let diagnostics = run(sql);

            assert!(
                diagnostics.is_empty(),
                "functions that do not reference columns should be allowed even when other branches compare columns"
            );
        }

        #[test]
        fn detects_function_on_both_sides() {
            let sql = "SELECT * FROM users u1 JOIN users u2 ON trim(u1.email) = trim(u2.email);";
            let diagnostics = run(sql);

            assert!(diagnostics
                .iter()
                .all(|diag| diag.rule_id == "no-function-on-column-in-join-or-where"),);

            assert_eq!(
                diagnostics.len(),
                2,
                "expected two diagnostics for functions on both sides"
            );

            let spans: Vec<_> = diagnostics
                .iter()
                .map(|diag| &sql[diag.span.start.byte..diag.span.end.byte])
                .collect();
            assert!(
                spans.iter().any(|s| s.contains("trim(u1.email)")),
                "expected trim() function on left side to be detected"
            );
            assert!(
                spans.iter().any(|s| s.contains("trim(u2.email)")),
                "expected trim() function on right side to be detected"
            );
        }
    }

    mod join_qual {

        use super::*;

        #[test]
        fn allows_function_with_constant() {
            let sql =
                "SELECT * FROM t1 JOIN t2 ON t1.created_at >= to_date('20160101', 'YYYYMMDD');";
            let diagnostics = run(sql);

            assert!(diagnostics.is_empty(),);
        }

        #[test]
        fn detects_function_in_join_condition() {
            let sql = "SELECT * FROM t1 JOIN t2 ON lower(t1.name) = t2.name;";
            let diagnostics = run(sql);

            assert!(diagnostics
                .iter()
                .any(|diag| diag.rule_id == "no-function-on-column-in-join-or-where"),);

            assert_eq!(diagnostics.len(), 1);

            let SqlSpan { start, end } = diagnostics[0].span;
            assert_eq!(&sql[start.byte..end.byte], "lower(t1.name)");
        }

        #[test]
        fn detects_function_on_both_sides() {
            let sql = "SELECT * FROM t1 JOIN t2 ON trim(t1.email) = trim(t2.email);";
            let diagnostics = run(sql);

            assert!(diagnostics
                .iter()
                .all(|diag| diag.rule_id == "no-function-on-column-in-join-or-where"),);

            assert_eq!(diagnostics.len(), 2,);

            let spans: Vec<_> = diagnostics
                .iter()
                .map(|diag| &sql[diag.span.start.byte..diag.span.end.byte])
                .collect();
            assert!(
                spans.iter().any(|s| s.contains("trim(t1.email)")),
                "expected trim() function on left side to be detected"
            );
            assert!(
                spans.iter().any(|s| s.contains("trim(t2.email)")),
                "expected trim() function on right side to be detected"
            );
        }
    }

    mod other_location {
        use super::*;

        #[test]
        fn allows_function_in_select_column() {
            let sql = "SELECT func(col) FROM tbl;";
            let diagnostics = run(sql);

            assert!(diagnostics.is_empty(),);
        }

        #[test]
        fn allows_function_in_subquery() {
            let sql = "SELECT * FROM tbl WHERE col IN (SELECT func(col) FROM tbl);";
            let diagnostics = run(sql);

            assert!(diagnostics.is_empty(),);
        }
    }
}
