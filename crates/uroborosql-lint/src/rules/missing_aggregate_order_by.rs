use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

const RULE_NAME: &str = "missing-aggregate-order-by";

/// Order-dependent aggregate functions checked by this rule.
const ORDER_DEPENDENT_AGGREGATES: &[&str] = &[
    "array_agg",
    "string_agg",
    "json_agg",
    "jsonb_agg",
    "xmlagg",
    // 生成物は順序依存だが、JSONとしてparseすると、key重複がなければ等価.
    // "json_object_agg",
    // "jsonb_object_agg",
];

/// Detects order-dependent aggregate functions called without `ORDER BY`.
///
/// Aggregates used as window functions (`OVER`) and ordered-set aggregates (`WITHIN GROUP`)
/// are not checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingAggregateOrderBy;

impl Rule for MissingAggregateOrderBy {
    fn name(&self) -> &'static str {
        RULE_NAME
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::func_expr]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(range) = detect(node) else {
            return;
        };

        let diagnostic = Diagnostic::new(
            self.name(),
            severity,
            "Order-dependent aggregate without ORDER BY returns elements in an undefined order; add ORDER BY inside the call.",
            &range,
        );
        ctx.report(diagnostic);
    }
}

/// Normalizes an identifier.
fn normalize_identifier(text: &str) -> String {
    match text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) {
        // "array_agg" => array_agg
        Some(inner) => inner.replace("\"\"", "\""),
        // ARRAY_AGG => array_agg
        None => text.to_ascii_lowercase(),
    }
}

/// Returns the range of the aggregate call when it is order-dependent and has no `ORDER BY`.
fn detect(func_expr: &Node<'_>) -> Option<Range> {
    // func_expr
    // - func_application within_group_clause? filter_clause? over_clause?
    let func_application = func_expr.first_child()?;
    if func_application.kind() != SyntaxKind::func_application {
        return None;
    }

    if func_expr.children().iter().any(|child| {
        matches!(
            child.kind(),
            SyntaxKind::over_clause | SyntaxKind::within_group_clause
        )
    }) {
        return None;
    }

    // func_application
    // - func_name '(' ... sort_clause? ')'
    let func_name = func_application.first_child()?;
    if func_name.kind() != SyntaxKind::func_name {
        return None;
    }

    // Use the last identifier so that `pg_catalog.array_agg` is checked as `array_agg`.
    let name = normalize_identifier(func_name.last_token()?.text());
    if !ORDER_DEPENDENT_AGGREGATES.contains(&name.as_str()) {
        return None;
    }

    if func_application
        .children()
        .iter()
        .any(|child| child.kind() == SyntaxKind::sort_clause)
    {
        return None;
    }

    Some(func_application.range())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{linter::tests::run_with_rules, rules::RuleEnum, SqlSpan};

    fn run(sql: &str) -> Vec<crate::Diagnostic> {
        run_with_rules(
            sql,
            vec![RuleEnum::MissingAggregateOrderBy(MissingAggregateOrderBy)],
        )
    }

    #[test]
    fn detects_json_agg_without_order_by() {
        let sql = "SELECT json_agg(item_name) FROM items;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected json_agg without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "json_agg(item_name)");
    }

    #[test]
    fn detects_distinct_without_order_by() {
        let sql = "SELECT category, array_agg(DISTINCT item_name) FROM items GROUP BY category;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected array_agg(DISTINCT ...) without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "array_agg(DISTINCT item_name)");
    }

    #[test]
    fn detects_xmlagg_without_order_by() {
        let sql = "SELECT xmlagg(xmlelement(name item, item_name)) FROM items WHERE price > 0;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected xmlagg without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(
            &sql[start.byte..end.byte],
            "xmlagg(xmlelement(name item, item_name))"
        );
    }

    #[test]
    fn detects_string_agg_without_order_by() {
        let sql = "SELECT string_agg(item_name, ',') FROM items;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected string_agg without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "string_agg(item_name, ',')");
    }

    #[test]
    fn detects_jsonb_agg_without_order_by() {
        let sql = "SELECT jsonb_agg(item_name) FROM items;";
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected jsonb_agg without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], "jsonb_agg(item_name)");
    }

    #[test]
    fn allows_string_agg_with_order_by() {
        let sql = "SELECT string_agg(item_name, ',' ORDER BY item_name) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_jsonb_agg_with_order_by() {
        let sql = "SELECT jsonb_agg(item_name ORDER BY item_name) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn detects_quoted_aggregate_name_without_order_by() {
        let sql = r#"SELECT "array_agg"(item_name) FROM items;"#;
        let diagnostics = run(sql);

        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.code == RULE_NAME)
            .expect("expected quoted array_agg without ORDER BY to be detected");

        let SqlSpan { start, end } = diagnostic.span;
        assert_eq!(&sql[start.byte..end.byte], r#""array_agg"(item_name)"#);
    }

    #[test]
    fn detects_quoted_schema_qualified_aggregate_name() {
        let sql = r#"SELECT pg_catalog."array_agg"(item_name) FROM items;"#;
        assert_eq!(run(sql).len(), 1);
    }

    #[test]
    fn allows_quoted_aggregate_name_with_order_by() {
        let sql = r#"SELECT "array_agg"(item_name ORDER BY item_name) FROM items;"#;
        assert!(run(sql).is_empty());
    }

    #[test]
    fn ignores_quoted_uppercase_name() {
        // A quoted identifier is case-sensitive, so "ARRAY_AGG" is not the built-in array_agg.
        let sql = r#"SELECT "ARRAY_AGG"(item_name) FROM items;"#;
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_json_agg_with_order_by() {
        let sql = "SELECT json_agg(item_name ORDER BY item_name) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_distinct_with_order_by() {
        let sql = "SELECT category, array_agg(DISTINCT item_name ORDER BY item_name) FROM items GROUP BY category;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_xmlagg_with_order_by() {
        let sql =
            "SELECT xmlagg(xmlelement(name item, item_name) ORDER BY id) FROM items WHERE price > 0;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_order_independent_aggregate() {
        let sql = "SELECT sum(price) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_aggregate_with_over() {
        let sql = "SELECT array_agg(item_name) OVER (PARTITION BY category) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_aggregate_with_within_group() {
        let sql = "SELECT array_agg(item_name) WITHIN GROUP (ORDER BY item_name) FROM items;";
        assert!(run(sql).is_empty());
    }
}
