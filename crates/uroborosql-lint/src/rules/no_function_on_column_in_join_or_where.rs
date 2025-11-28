use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

/// Detects functions use on columns in JOIN or WHERE conditions.
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
        let func_expr = node;
        
        // 親を参照し、 join_qual か where_clause があるかをチェックする
        // その途中で select_no_parens があれば探索を停止する
        if !is_in_detection_range(func_expr) {
            return;
        }
        
        // func_expr の最初の子供は func_application, json_aggregate_func, または func_expr_common_subexpr のいずれかである
        let function_body = func_expr.first_child().expect("func_expr should have one of func_application, json_aggregate_func, or func_expr_common_subexpr as its first child.");
        
        // 引数にカラムがあるか判定
        
        // 警告範囲には filter や over を含めない
        // func_expr の最初の子供の範囲とする
        

        unimplemented!()

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Functions in JOIN or WHERE conditions can prevent index usage; rewrite without wrapping the column.",
            &range,
        );
        ctx.report(diagnostic);
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
//   - func_name '(' ')'
//   - func_name '(' func_arg_list opt_sort_clause ')'
//   - func_name '(' VARIADIC func_arg_expr opt_sort_clause ')'
//   - func_name '(' func_arg_list ',' VARIADIC func_arg_expr opt_sort_clause ')'
//   - func_name '(' ALL func_arg_list opt_sort_clause ')'
//   - func_name '(' DISTINCT func_arg_list opt_sort_clause ')'
//   - func_name '(' '*' ')'
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
//   - nothing
//   - a_expr
//   - extract_list (EXTRACT '(' extract_list ')')
//   - overlay_list (OVERLAY '(' overlay_list ')')
//   - func_arg_list_opt (func_name '(' func_arg_list_opt ')')
//   - func_arg_list
//   - position_list (POSITION '(' position_list ')')
//   - substr_list (SUBSTRING '(' substr_list ')')
//   - trim_list (TRIM '(' trim_list ')')
//   - expr_list
//   - xmlexists_argument (XMLEXISTS '(' c_expr xmlexists_argument ')')
//   - c_expr
//   - xml_attribute_list (XMLFOREST '(' xml_attribute_list ')')



//   - JSON_OBJECT '(' json_name_and_value_list json_object_constructor_null_clause_opt json_key_uniqueness_constraint_opt json_returning_clause_opt ')'
//   - JSON_OBJECT '(' json_returning_clause_opt ')'
//   - JSON_ARRAY '(' json_value_expr_list json_array_constructor_null_clause_opt json_returning_clause_opt ')'
//   - JSON_ARRAY '(' select_no_parens json_format_clause_opt json_returning_clause_opt ')'
//   - JSON_ARRAY '(' json_returning_clause_opt ')'
//   - JSON '(' json_value_expr json_key_uniqueness_constraint_opt ')'
//   - JSON_SERIALIZE '(' json_value_expr json_returning_clause_opt ')'
//   - JSON_QUERY '(' json_value_expr ',' a_expr json_passing_clause_opt json_returning_clause_opt json_wrapper_behavior json_quotes_clause_opt json_behavior_clause_opt ')'
//   - JSON_EXISTS '(' json_value_expr ',' a_expr json_passing_clause_opt json_on_error_clause_opt ')'
//   - JSON_VALUE '(' json_value_expr ',' a_expr json_passing_clause_opt json_returning_clause_opt json_behavior_clause_opt ')'


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

fn is_in_detection_range(func_expr: &Node) -> bool {
    // 親を辿り、 join_qual か where_clause があるかを検証する
    // その途中で select_no_parens があれば探索を停止する

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

// 引数にカラムを含むかどうか
fn has_column_argument(func_expr: &Node) -> bool {
    unimplemented!()
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
