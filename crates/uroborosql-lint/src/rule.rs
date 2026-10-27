use crate::{context::LintContext, diagnostic::Severity};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[]
    }
    /// Catalog rules run only once catalog state is in the context.
    fn needs_catalog(&self) -> bool {
        false
    }
    fn run_once<'tree>(&self, _root: &Node<'tree>, _ctx: &mut LintContext, _severity: Severity) {}
    fn run_on_node<'tree>(&self, _node: &Node<'tree>, _ctx: &mut LintContext, _severity: Severity) {
    }
}
