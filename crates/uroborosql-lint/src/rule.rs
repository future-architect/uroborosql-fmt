use crate::{context::LintContext, diagnostic::Severity};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn severity(&self) -> Severity;
    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[]
    }
    fn run_once<'tree>(&self, _root: &Node<'tree>, _ctx: &mut LintContext) {}
    fn run_on_node<'tree>(&self, _node: &Node<'tree>, _ctx: &mut LintContext) {}
}
