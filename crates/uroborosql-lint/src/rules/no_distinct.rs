use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

use crate::{
    context::LintContext,
    diagnostic::{Diagnostic, Severity},
    rule::Rule,
};

pub struct NoDistinct;

impl Rule for NoDistinct {
    fn name(&self) -> &'static str {
        "no-distinct"
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[SyntaxKind::DISTINCT]
    }

    fn run_on_node<'tree>(&self, node: &Node<'tree>, ctx: &mut LintContext) {
        let diagnostic = Diagnostic::new(
            self.name(),
            self.severity(),
            "DISTINCT is prohibited by project guidelines",
            &node.range(),
        );
        ctx.report(diagnostic);
    }
}
