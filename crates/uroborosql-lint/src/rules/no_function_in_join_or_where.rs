use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

/// Detects functions use  in JOIN or WHERE conditions.
/// source: https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#:~:text=1-,%E3%82%A4%E3%83%B3%E3%83%87%E3%83%83%E3%82%AF%E3%82%B9%E3%82%AB%E3%83%A9%E3%83%A0%E3%81%AB%E9%96%A2%E6%95%B0,-%E3%82%92%E9%80%9A%E3%81%97%E3%81%9F
pub struct NoFunctionInJoinOrWhere;

impl Rule for NoFunctionInJoinOrWhere {
    fn name(&self) -> &'static str {
        "no-function-in-join-or-where"
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::join_qual, SyntaxKind::where_clause]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        assert!(matches!(
            node.kind(),
            SyntaxKind::join_qual | SyntaxKind::where_clause
        ));

        let Some(top_expr) = find_top_expr(node) else {
            return;
        };

        let ranges = detect_column_function_calls(&top_expr);

        for range in ranges {
            let diagnostic = Diagnostic::new(
                self.name(),
                severity,
                "Functions in JOIN or WHERE conditions can prevent index usage; rewrite without wrapping the column.",
                &range,
            );
            ctx.report(diagnostic);
        }
    }
}

/// Finds the top `a_expr` in a JOIN or WHERE clause.
fn find_top_expr<'a>(join_qual_or_where_clause: &'a Node<'a>) -> Option<Node<'a>> {
    // join_qual:
    // - ON a_expr
    // - USING '(' name_list ')' opt_alias_clause_for_joiln_using
    //
    // where_clause:
    // - a_expr

    let last_child = join_qual_or_where_clause
        .last_child()
        .expect("join_qual or where_clause must have a last child.");

    if last_child.kind() == SyntaxKind::a_expr {
        Some(last_child)
    } else {
        None
    }
}

// a_expr の子孫に func_expr_windowless が出現することはない
//
// その他関数系ノード
// - func_expr
//   - func_application within_group_clause filter_clause
//   - json_aggregate_func filter_clause over_clause
//   - func_expr_common_subexpr
//
// - func_application
//   - カラムが出現しうる箇所は子供のうち func_arg_list か func_arg_expr を見れば良さそう
//
// - json_aggregate_func
//   - JSON_ARRAY_AGG '(' json_value_expr_list json_array_constructor_null_clause_opt json_returning_clause_opt
//     - json_value_expr_list
//       - json_value_expr
//         - a_expr json_format_clause_opt
//   - JSON_OBJECT_AGG '(' json_name_and_value json_object_constructor_null_clause_opt json_key_uniqueness_constraint_opt json_returning_clause_opt
//     - json_name_and_value
//       - c_expr VALUE_P json_value_expr
//       - a_expr ':' json_value_expr
//
// - func_expr_common_subexpr
//
// a_expr の子孫で func_expr が現れるまでの最短ルート
// a_expr
// - c_expr
//   - func_expr

const FUNCTION_KINDS: &[SyntaxKind] = &[
    SyntaxKind::func_application,
    SyntaxKind::func_expr_common_subexpr,
    SyntaxKind::json_aggregate_func,
];

// SELECT サブクエリ以下は JOIN / WHERE の外側なので探索対象外。
const SUBQUERY_KINDS: &[SyntaxKind] =
    &[SyntaxKind::select_with_parens, SyntaxKind::select_no_parens];

fn detect_column_function_calls(a_expr: &Node<'_>) -> Vec<Range> {
    let mut ranges = Vec::new();
    let mut stack = vec![a_expr.clone()];

    while let Some(current) = stack.pop() {
        // 対象となる関数ノードを見つけたら、直下に列参照があるかを判定する。
        // ネストしている場合は「列に最も近い（内側の）関数だけ」を診断対象にする。
        if FUNCTION_KINDS.contains(&current.kind())
            && contains_columnref_excluding_nested_functions(&current)
        {
            ranges.push(current.range());
        }
        push_child_nodes(&mut stack, &current);
    }

    ranges
}

fn contains_columnref_excluding_nested_functions(node: &Node<'_>) -> bool {
    let mut stack = Vec::new();
    push_child_nodes(&mut stack, node);

    while let Some(current) = stack.pop() {
        if current.kind() == SyntaxKind::columnref {
            return true;
        }
        // 内側に別の関数が現れたら、そちらで改めて判定するためここでは追わない。
        if FUNCTION_KINDS.contains(&current.kind()) {
            continue;
        }
        push_child_nodes(&mut stack, &current);
    }

    false
}

fn push_child_nodes<'tree>(stack: &mut Vec<Node<'tree>>, node: &Node<'tree>) {
    if SUBQUERY_KINDS.contains(&node.kind()) {
        return;
    }

    for child in node.children() {
        if child.child_count() == 0 {
            continue;
        }
        stack.push(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, Diagnostic, SqlSpan};

    fn run(sql: &str) -> Vec<Diagnostic> {
        run_with_rules(sql, vec![Box::new(NoFunctionInJoinOrWhere)])
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
                .any(|diag| diag.rule_id == "no-function-in-join-or-where"),);

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
                .all(|diag| diag.rule_id == "no-function-in-join-or-where"),);

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
                .any(|diag| diag.rule_id == "no-function-in-join-or-where"),);

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
                .all(|diag| diag.rule_id == "no-function-in-join-or-where"),);

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
