use crate::{
    context::LintContext, diagnostic::Diagnostic, directive::suppress_diagnostics,
    tree::collect_preorder, ResolvedLintConfig,
};
use postgresql_cst_parser::{tree_sitter, ParserError, ScanReport};

#[derive(Debug)]
pub enum LintError {
    ParseError {
        message: String,
        /// `None` when the position is unavailable.
        span: Option<ParseErrorByteSpan>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseErrorByteSpan {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl LintError {
    fn from_parser_error(err: ParserError) -> Self {
        match err {
            ParserError::ParseError {
                message,
                start_byte_pos,
                end_byte_pos,
            } => LintError::ParseError {
                message,
                span: Some(ParseErrorByteSpan {
                    start_byte: start_byte_pos,
                    end_byte: end_byte_pos,
                }),
            },
            ParserError::ScanReport(ScanReport {
                message,
                position_in_bytes,
                ..
            }) => LintError::ParseError {
                message,
                span: Some(ParseErrorByteSpan {
                    start_byte: position_in_bytes,
                    end_byte: position_in_bytes,
                }),
            },
            ParserError::ScanError { message } => LintError::ParseError {
                message,
                span: None,
            },
        }
    }
}

#[derive(Debug, Default)]
pub struct Linter;

impl Linter {
    pub fn new() -> Self {
        Self
    }

    pub fn run(
        &self,
        sql: &str,
        resolved_config: &ResolvedLintConfig,
    ) -> Result<Vec<Diagnostic>, LintError> {
        let tree = tree_sitter::parse_2way(sql).map_err(LintError::from_parser_error)?;
        let root = tree.root_node();
        let nodes = collect_preorder(root.clone());
        let mut ctx = LintContext::new(sql);

        for (rule, severity) in &resolved_config.rules {
            rule.run_once(&root, &mut ctx, *severity);

            let targets = rule.target_kinds();
            if targets.is_empty() {
                for node in &nodes {
                    rule.run_on_node(node, &mut ctx, *severity);
                }
            } else {
                for node in &nodes {
                    if targets.iter().any(|kind| node.kind() == *kind) {
                        rule.run_on_node(node, &mut ctx, *severity);
                    }
                }
            }
        }

        Ok(suppress_diagnostics(&root, ctx.into_diagnostics()))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::{
        diagnostic::Severity,
        rules::{NoDistinct, NoWildcardProjection, RuleEnum},
    };

    fn resolve_from_rules(rules: Vec<(RuleEnum, Severity)>) -> ResolvedLintConfig {
        ResolvedLintConfig { rules, db: None }
    }

    pub fn run_with_rules(sql: &str, rules: Vec<RuleEnum>) -> Vec<Diagnostic> {
        let resolved_rules = rules
            .into_iter()
            .map(|rule| {
                let severity = rule.default_severity();
                (rule, severity)
            })
            .collect();
        let state = resolve_from_rules(resolved_rules);

        Linter::new().run(sql, &state).expect("lint ok")
    }

    #[test]
    fn applies_severity_override() {
        let resolved_config =
            resolve_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Error)]);
        let sql = "SELECT DISTINCT id FROM users;";
        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn disable_next_line_suppresses_only_the_next_physical_line() {
        let sql = r#"-- uroborosql-lint-disable-next-line no-distinct
SELECT DISTINCT id FROM users;
SELECT DISTINCT name FROM users;"#;

        let diagnostics = run_with_rules(sql, vec![RuleEnum::NoDistinct(NoDistinct)]);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.start.line, 2);
    }

    #[test]
    fn disable_next_line_does_not_skip_blank_lines() {
        let sql = r#"-- uroborosql-lint-disable-next-line no-distinct

SELECT DISTINCT id FROM users;"#;

        let diagnostics = run_with_rules(sql, vec![RuleEnum::NoDistinct(NoDistinct)]);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.start.line, 2);
    }

    #[test]
    fn file_head_disable_suppresses_requested_rule_only() {
        let sql = r#"-- uroborosql-lint-disable no-distinct
SELECT DISTINCT * FROM users;"#;

        let diagnostics = run_with_rules(
            sql,
            vec![
                RuleEnum::NoDistinct(NoDistinct),
                RuleEnum::NoWildcardProjection(NoWildcardProjection),
            ],
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "no-wildcard-projection");
    }

    #[test]
    fn file_head_disable_remains_effective_after_block_comment() {
        let sql = r#"-- uroborosql-lint-disable no-distinct
/* comment */
SELECT DISTINCT id FROM users;"#;

        let diagnostics = run_with_rules(sql, vec![RuleEnum::NoDistinct(NoDistinct)]);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn disable_after_block_comment_is_ignored() {
        let sql = r#"/* comment */
-- uroborosql-lint-disable no-distinct
SELECT DISTINCT id FROM users;"#;

        let diagnostics = run_with_rules(sql, vec![RuleEnum::NoDistinct(NoDistinct)]);

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn file_head_disable_and_next_line_directives_compose() {
        let sql = r#"-- uroborosql-lint-disable no-distinct
-- uroborosql-lint-disable-next-line no-wildcard-projection
SELECT DISTINCT * FROM users;"#;

        let diagnostics = run_with_rules(
            sql,
            vec![
                RuleEnum::NoDistinct(NoDistinct),
                RuleEnum::NoWildcardProjection(NoWildcardProjection),
            ],
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn linter_run_returns_suppressed_diagnostics() {
        let resolved_config =
            resolve_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)]);
        let sql = r#"-- uroborosql-lint-disable no-distinct
SELECT DISTINCT id FROM users;"#;

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn unknown_rule_in_directive_produces_warning_on_rule_name() {
        let resolved_config =
            resolve_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)]);
        let sql = r#"-- uroborosql-lint-disable definitely-not-a-rule
SELECT DISTINCT id FROM users;"#;

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "invalid-lint-directive");
        assert_eq!(
            diagnostics[0].message,
            "unknown lint directive rule `definitely-not-a-rule`"
        );
        assert_eq!(diagnostics[0].span.start.line, 0);
        assert_eq!(diagnostics[0].span.start.column, 27);
        assert_eq!(diagnostics[0].span.end.column, 48);
        assert_eq!(diagnostics[1].code, "no-distinct");
    }

    #[test]
    fn unknown_rule_does_not_prevent_known_rules_from_being_suppressed() {
        let resolved_config =
            resolve_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)]);
        let sql = r#"-- uroborosql-lint-disable no-distinct, definitely-not-a-rule
SELECT DISTINCT id FROM users;"#;

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "invalid-lint-directive");
        assert_eq!(
            diagnostics[0].message,
            "unknown lint directive rule `definitely-not-a-rule`"
        );
    }

    #[test]
    fn malformed_rule_token_produces_unknown_rule_warning() {
        let resolved_config =
            resolve_from_rules(vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)]);
        let sql = r#"-- uroborosql-lint-disable no-distinct because reason
SELECT DISTINCT id FROM users;"#;

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, "invalid-lint-directive");
        assert_eq!(
            diagnostics[0].message,
            "unknown lint directive rule `no-distinct because reason`"
        );
        assert_eq!(diagnostics[1].code, "no-distinct");
    }

    #[test]
    fn parse_error_carries_byte_span_at_error_location() {
        let cfg = resolve_from_rules(vec![]);
        // WHERE has no condition, so it errors at EOF.
        let err = Linter::new()
            .run("SELECT id FROM users WHERE", &cfg)
            .expect_err("should fail to parse");

        let LintError::ParseError { span, .. } = err;
        let span = span.expect("parse error should carry a byte span");
        assert_eq!(span.start_byte, 26);
        assert_ne!(span.start_byte, 0);
    }

    #[test]
    fn parse_error_span_points_at_offending_token() {
        let cfg = resolve_from_rules(vec![]);
        // `+` has no right operand, so it errors at the `;`.
        let err = Linter::new()
            .run("SELECT 1 + ;", &cfg)
            .expect_err("should fail to parse");

        let LintError::ParseError { span, .. } = err;
        let span = span.expect("parse error should carry a byte span");
        assert_eq!(span.start_byte, 11);
        assert_eq!(span.end_byte, 12);
    }
}
