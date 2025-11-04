use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};
use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Range},
};

/// Detects wildcard projections. (e.g. `SELECT *`, `SELECT u.*`, `RETURNING *`)
/// Rule source: https://future-architect.github.io/coding-standards/documents/forSQL/SQL%E3%82%B3%E3%83%BC%E3%83%87%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%8F%E7%B4%84%EF%BC%88PostgreSQL%EF%BC%89.html#%E6%A4%9C%E7%B4%A2:~:text=%E3%82%92%E6%8C%87%E5%AE%9A%E3%81%99%E3%82%8B-,%E5%85%A8%E5%88%97%E3%83%AF%E3%82%A4%E3%83%AB%E3%83%89%E3%82%AB%E3%83%BC%E3%83%89%E3%80%8C*%E3%80%8D%E3%81%AE%E4%BD%BF%E7%94%A8%E3%81%AF%E3%81%9B%E3%81%9A%E3%80%81%E3%82%AB%E3%83%A9%E3%83%A0%E5%90%8D%E3%82%92%E6%98%8E%E8%A8%98%E3%81%99%E3%82%8B,-%E3%82%A4%E3%83%B3%E3%83%87%E3%83%83%E3%82%AF%E3%82%B9%E3%81%AB%E3%82%88%E3%82%8B%E6%A4%9C%E7%B4%A2
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

fn detect_wildcard(target_el_node: &Node<'_>) -> Option<Range> {
    assert_eq!(target_el_node.kind(), SyntaxKind::target_el);
    // target_el:
    // - '*'
    //   ^^^ Wildcard
    // - a_expr AS ColLabel
    // - a_expr BareColLabel
    // - a_expr
    //   ^^^^^^ If a_expr contains a columnref, a wildcard may appear
    //
    // source: https://github.com/postgres/postgres/blob/65f4976189b6cbe9aa93fc5f4b1eb7a2040b6301/src/backend/parser/gram.y#L17364-L17401

    let mut cursor = target_el_node.walk();
    cursor.goto_first_child();

    match cursor.node().kind() {
        SyntaxKind::Star => Some(cursor.node().range()),
        SyntaxKind::a_expr => {
            let a_expr = cursor.node();
            let columnref = get_columnref_from_a_expr(&a_expr)?;

            // columnref:
            // - ColId
            // - ColId indirection
            //   - e.g.: `a.field`, `a.field[1]`, `a.*`
            //
            // source: https://github.com/postgres/postgres/blob/65f4976189b6cbe9aa93fc5f4b1eb7a2040b6301/src/backend/parser/gram.y#L17041-L17049

            let indirection = columnref
                .children()
                .iter()
                .find(|child| child.kind() == SyntaxKind::indirection)
                .cloned()?;

            // indirection: list of indirection_el
            let last_indirection_el = indirection.children().last()?.clone();

            // indirection_el:
            // - '.' attr_name
            // - '.' '*'
            // - '[' a_expr ']'
            // - '[' opt_slice_bound ':' opt_slice_bound ']'
            //
            // source: https://github.com/postgres/postgres/blob/65f4976189b6cbe9aa93fc5f4b1eb7a2040b6301/src/backend/parser/gram.y#L17051-L17078
            let last_child = last_indirection_el.children().last().cloned()?;

            // possible: attr_name, '*', ']'
            if last_child.kind() == SyntaxKind::Star {
                Some(last_indirection_el.range())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Retrieves the `columnref` node from an `a_expr` node if it exists.
fn get_columnref_from_a_expr<'a>(a_expr: &'a Node<'a>) -> Option<Node<'a>> {
    // current: a_expr
    //
    // possible structure:
    // - a_expr
    //   - c_expr
    //     - columnref

    let mut cursor = a_expr.walk();
    cursor.goto_first_child();

    match cursor.node().kind() {
        SyntaxKind::c_expr => {
            cursor.goto_first_child();
            if cursor.node().kind() == SyntaxKind::columnref {
                return Some(cursor.node());
            }

            None
        }
        _ => None,
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
