use crate::{
    context::LintContext,
    diagnostic::Severity,
    rules::{
        MissingTwoWaySample, NoDistinct, NoNotIn, NoUnionDistinct, NoWildcardProjection,
        TooLargeInList,
    },
};
use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

pub trait Rule: Send + Sync {
    fn name(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    fn target_kinds(&self) -> &'static [SyntaxKind] {
        &[]
    }
    fn run_once<'tree>(&self, _root: &Node<'tree>, _ctx: &mut LintContext, _severity: Severity) {}
    fn run_on_node<'tree>(&self, _node: &Node<'tree>, _ctx: &mut LintContext, _severity: Severity) {
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleEnum {
    NoDistinct(NoDistinct),
    NoNotIn(NoNotIn),
    NoUnionDistinct(NoUnionDistinct),
    NoWildcardProjection(NoWildcardProjection),
    MissingTwoWaySample(MissingTwoWaySample),
    TooLargeInList(TooLargeInList),
}
