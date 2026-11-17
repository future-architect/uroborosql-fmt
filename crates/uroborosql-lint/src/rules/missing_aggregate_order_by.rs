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
    // `json_object_agg` and `jsonb_object_agg` are not checked: the objects they build are
    // equal as JSON regardless of the input order, unless keys collide.
];

/// Operators whose result does not depend on the element order of an array operand.
const ORDER_INDEPENDENT_ARRAY_OPERATORS: &[&str] = &["@>", "<@", "&&"];

/// One-argument functions whose result does not depend on the element order of the array.
const ORDER_INDEPENDENT_UNARY_ARRAY_FUNCTIONS: &[&str] =
    &["cardinality", "array_ndims", "array_dims"];

/// Two-argument functions (array, dimension) whose result does not depend on the element order
/// of the array, which is the first argument.
const ORDER_INDEPENDENT_BINARY_ARRAY_FUNCTIONS: &[&str] =
    &["array_length", "array_lower", "array_upper"];

/// Detects order-dependent aggregate functions called without `ORDER BY`.
///
/// Aggregates used as window functions (`OVER`) and ordered-set aggregates (`WITHIN GROUP`)
/// are not checked. The result of `array_agg` is also not checked when it is consumed in a way
/// that ignores the element order.
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

/// Returns the parent of `node` if it is of the given kind.
fn parent_of_kind<'tree>(node: &Node<'tree>, kind: SyntaxKind) -> Option<Node<'tree>> {
    node.parent().filter(|parent| parent.kind() == kind)
}

/// `func_application` calls one of `names`.
///
/// Uses the last identifier so that `pg_catalog.cardinality` is treated as `cardinality`.
fn function_name_is(func_application: &Node<'_>, names: &[&str]) -> bool {
    func_application
        .first_child()
        .filter(|func_name| func_name.kind() == SyntaxKind::func_name)
        .and_then(|func_name| func_name.last_token())
        .is_some_and(|token| names.contains(&normalize_identifier(token.text()).as_str()))
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

    if name == "array_agg" && is_consumed_order_independently(func_expr) {
        return None;
    }

    Some(func_application.range())
}

/// Returns true when the array produced by `func_expr` is used where its element order
/// cannot affect the result.
fn is_consumed_order_independently(func_expr: &Node<'_>) -> bool {
    // Skip every wrapper that passes the array through, however many there are.
    let mut operand = *func_expr;
    while let Some(parent) = transparent_parent(&operand) {
        operand = parent;
    }

    operand
        .parent()
        .is_some_and(|consumer| is_order_independent_consumer(&consumer, &operand))
}

/// `consumer` uses `operand` (an array) in a way that ignores its element order.
fn is_order_independent_consumer(consumer: &Node<'_>, operand: &Node<'_>) -> bool {
    let children = consumer.children();

    match consumer.kind() {
        // a_expr subquery_Op sub_type '(' a_expr ')'
        SyntaxKind::a_expr
            if children.len() == 6
                && children[1].kind() == SyntaxKind::subquery_Op
                && children[2].kind() == SyntaxKind::sub_type =>
        {
            children[4] == *operand
        }
        // a_expr qual_Op a_expr
        SyntaxKind::a_expr if children.len() == 3 && children[1].kind() == SyntaxKind::qual_Op => {
            ORDER_INDEPENDENT_ARRAY_OPERATORS.contains(&children[1].text())
        }
        SyntaxKind::func_arg_expr => is_order_independent_function_argument(consumer),
        _ => false,
    }
}

/// Returns the parent of `node` when it passes the value through without depending on the
/// element order.
fn transparent_parent<'tree>(node: &Node<'tree>) -> Option<Node<'tree>> {
    let parent = node.parent()?;
    let children = parent.children();

    let transparent = match parent.kind() {
        // Nodes that only wrap their single child.
        SyntaxKind::a_expr | SyntaxKind::c_expr | SyntaxKind::func_expr if children.len() == 1 => {
            true
        }
        // c_expr: '(' a_expr ')'
        SyntaxKind::c_expr => children.len() == 3 && children[0].kind() == SyntaxKind::LParen,
        _ => false,
    };

    transparent.then_some(parent)
}

/// `func_arg_expr` is the array argument of an order-independent array function:
/// the only argument of a unary function, or the first argument of a binary one.
fn is_order_independent_function_argument(func_arg_expr: &Node<'_>) -> bool {
    // func_application
    // - func_name '(' func_arg_list ')'
    //   - func_arg_list is flat: func_arg_expr (',' func_arg_expr)*
    parent_of_kind(func_arg_expr, SyntaxKind::func_arg_list).is_some_and(|list| {
        let args = list.children();
        let names = match args.len() {
            1 => ORDER_INDEPENDENT_UNARY_ARRAY_FUNCTIONS,
            // func_arg_expr ',' func_arg_expr
            3 if args[0] == *func_arg_expr => ORDER_INDEPENDENT_BINARY_ARRAY_FUNCTIONS,
            _ => return false,
        };

        parent_of_kind(&list, SyntaxKind::func_application)
            .is_some_and(|call| function_name_is(&call, names))
    })
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
        .into_iter()
        .filter(|diag| diag.code == RULE_NAME)
        .collect()
    }

    #[test]
    fn detects_json_agg_without_order_by() {
        let sql = "SELECT json_agg(item_name) FROM items;";
        let diagnostics = run(sql);

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
        assert_eq!(&sql[start.byte..end.byte], "json_agg(item_name)");
    }

    #[test]
    fn detects_distinct_without_order_by() {
        let sql = "SELECT category, array_agg(DISTINCT item_name) FROM items GROUP BY category;";
        let diagnostics = run(sql);

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
        assert_eq!(&sql[start.byte..end.byte], "array_agg(DISTINCT item_name)");
    }

    #[test]
    fn detects_xmlagg_without_order_by() {
        let sql = "SELECT xmlagg(xmlelement(name item, item_name)) FROM items WHERE price > 0;";
        let diagnostics = run(sql);

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
        assert_eq!(
            &sql[start.byte..end.byte],
            "xmlagg(xmlelement(name item, item_name))"
        );
    }

    #[test]
    fn detects_string_agg_without_order_by() {
        let sql = "SELECT string_agg(item_name, ',') FROM items;";
        let diagnostics = run(sql);

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
        assert_eq!(&sql[start.byte..end.byte], "string_agg(item_name, ',')");
    }

    #[test]
    fn detects_jsonb_agg_without_order_by() {
        let sql = "SELECT jsonb_agg(item_name) FROM items;";
        let diagnostics = run(sql);

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
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

        assert_eq!(diagnostics.len(), 1);

        let SqlSpan { start, end } = diagnostics[0].span;
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

    #[test]
    fn allows_array_agg_in_any() {
        let sql =
            "SELECT category FROM items GROUP BY category HAVING 'a' = ANY(array_agg(item_name));";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_array_agg_in_contains_operator() {
        let sql =
            "SELECT category FROM items GROUP BY category HAVING array_agg(item_name) @> ARRAY['a', 'b'];";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_array_agg_in_overlap_operator_on_right() {
        let sql = "SELECT category FROM items GROUP BY category HAVING ARRAY['a'] && array_agg(item_name);";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_array_agg_in_order_independent_function() {
        let sql = "SELECT cardinality(array_agg(item_name)) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn allows_array_agg_as_first_argument_of_array_length() {
        let sql = "SELECT array_length(array_agg(item_name), 1) FROM items;";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn detects_array_agg_in_equality() {
        let sql = "SELECT category FROM items GROUP BY category HAVING array_agg(item_name) = ARRAY['a'];";
        assert_eq!(run(sql).len(), 1);
    }

    #[test]
    fn detects_array_agg_in_order_dependent_function() {
        let sql = "SELECT array_to_string(array_agg(item_name), ',') FROM items;";
        assert_eq!(run(sql).len(), 1);
    }

    #[test]
    fn detects_array_agg_with_subscript() {
        let sql = "SELECT array_append(ARRAY[1], (array_agg(id))[1]) FROM items;";
        assert_eq!(run(sql).len(), 1);
    }

    #[test]
    fn allows_array_agg_in_parentheses() {
        let sql = "SELECT category FROM items GROUP BY category HAVING 'a' = ANY((array_agg(item_name)));";
        assert!(run(sql).is_empty());
    }

    #[test]
    fn detects_array_agg_as_second_argument_of_array_length() {
        let sql = "SELECT array_length(ARRAY[1], array_agg(id)) FROM items;";
        assert_eq!(run(sql).len(), 1);
    }

    #[test]
    fn detects_array_agg_with_extra_argument_of_unary_function() {
        let sql = "SELECT cardinality(array_agg(id), 1) FROM items;";
        assert_eq!(run(sql).len(), 1);
    }
}
