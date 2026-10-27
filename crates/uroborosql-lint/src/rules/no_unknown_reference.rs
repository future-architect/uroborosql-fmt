use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    resolution::Finding,
    rule::Rule,
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

/// Reports tables and columns the acquired catalog proves absent; never acquires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoUnknownReference;

impl Rule for NoUnknownReference {
    fn name(&self) -> &'static str {
        "no-unknown-reference"
    }

    fn default_severity(&self) -> Severity {
        Severity::Error
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::relation_expr, SyntaxKind::columnref]
    }

    fn needs_catalog(&self) -> bool {
        true
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        let Some(catalog) = ctx.catalog() else {
            return;
        };
        let finding = match node.kind() {
            SyntaxKind::relation_expr => catalog.relation(node),
            _ => catalog.reference(node),
        };
        let (message, range) = match finding {
            Some(Finding::Table { spelling, range }) => {
                (format!("Table `{spelling}` does not exist."), range)
            }
            Some(Finding::Column { spelling, range }) => {
                (format!("Column `{spelling}` does not exist."), range)
            }
            Some(Finding::Qualifier { spelling, range }) => {
                (format!("Unknown qualifier `{spelling}`."), range)
            }
            None => return,
        };
        ctx.report(Diagnostic::new(self.name(), severity, message, &range));
    }
}
