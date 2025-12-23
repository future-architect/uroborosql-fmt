mod missing_two_way_sample;
mod no_distinct;
mod no_function_on_column_in_join_or_where;
mod no_not_in;
mod no_union_distinct;
mod no_wildcard_projection;
mod too_large_in_list;

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

use crate::{context::LintContext, diagnostic::Severity, rule::Rule};

pub use missing_two_way_sample::MissingTwoWaySample;
pub use no_distinct::NoDistinct;
pub use no_function_on_column_in_join_or_where::NoFunctionOnColumnInJoinOrWhere;
pub use no_not_in::NoNotIn;
pub use no_union_distinct::NoUnionDistinct;
pub use no_wildcard_projection::NoWildcardProjection;
pub use too_large_in_list::TooLargeInList;

pub fn all_rules() -> impl Iterator<Item = RuleEnum> {
    vec![
        RuleEnum::NoDistinct(NoDistinct),
        RuleEnum::NoNotIn(NoNotIn),
        RuleEnum::NoUnionDistinct(NoUnionDistinct),
        RuleEnum::NoWildcardProjection(NoWildcardProjection),
        RuleEnum::MissingTwoWaySample(MissingTwoWaySample),
        RuleEnum::TooLargeInList(TooLargeInList),
        RuleEnum::NoFunctionOnColumnInJoinOrWhere(NoFunctionOnColumnInJoinOrWhere),
    ]
    .into_iter()
}

pub fn default_rules() -> Vec<(RuleEnum, Severity)> {
    all_rules()
        .map(|rule| {
            let severity = rule.default_severity();
            (rule, severity)
        })
        .collect()
}

#[derive(Debug, Clone)]
pub enum RuleEnum {
    NoDistinct(NoDistinct),
    NoNotIn(NoNotIn),
    NoUnionDistinct(NoUnionDistinct),
    NoWildcardProjection(NoWildcardProjection),
    MissingTwoWaySample(MissingTwoWaySample),
    TooLargeInList(TooLargeInList),
    NoFunctionOnColumnInJoinOrWhere(NoFunctionOnColumnInJoinOrWhere),
}

impl RuleEnum {
    pub fn name(&self) -> &'static str {
        self.as_rule().name()
    }

    pub fn default_severity(&self) -> Severity {
        self.as_rule().default_severity()
    }

    pub fn target_kinds(&self) -> &'static [SyntaxKind] {
        self.as_rule().target_kinds()
    }

    pub fn run_once<'tree>(&self, root: &Node<'tree>, ctx: &mut LintContext, severity: Severity) {
        self.as_rule().run_once(root, ctx, severity);
    }

    pub fn run_on_node<'tree>(
        &self,
        node: &Node<'tree>,
        ctx: &mut LintContext,
        severity: Severity,
    ) {
        self.as_rule().run_on_node(node, ctx, severity);
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "no-distinct" => Some(Self::NoDistinct(NoDistinct)),
            "no-not-in" => Some(Self::NoNotIn(NoNotIn)),
            "no-union-distinct" => Some(Self::NoUnionDistinct(NoUnionDistinct)),
            "no-wildcard-projection" => Some(Self::NoWildcardProjection(NoWildcardProjection)),
            "missing-two-way-sample" => Some(Self::MissingTwoWaySample(MissingTwoWaySample)),
            "too-large-in-list" => Some(Self::TooLargeInList(TooLargeInList)),
            "no-function-on-column-in-join-or-where" => Some(
                Self::NoFunctionOnColumnInJoinOrWhere(NoFunctionOnColumnInJoinOrWhere),
            ),
            _ => None,
        }
    }

    fn as_rule(&self) -> &dyn Rule {
        match self {
            Self::NoDistinct(rule) => rule,
            Self::NoNotIn(rule) => rule,
            Self::NoUnionDistinct(rule) => rule,
            Self::NoWildcardProjection(rule) => rule,
            Self::MissingTwoWaySample(rule) => rule,
            Self::TooLargeInList(rule) => rule,
            Self::NoFunctionOnColumnInJoinOrWhere(rule) => rule,
        }
    }
}
