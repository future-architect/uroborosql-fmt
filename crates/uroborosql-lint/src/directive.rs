use std::collections::{HashMap, HashSet};

use postgresql_cst_parser::{syntax_kind::SyntaxKind, tree_sitter::Node};

use crate::{diagnostic::Diagnostic, rules::RuleEnum};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LintDirectiveKind {
    Disable,
    DisableNextLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LintDirective {
    kind: LintDirectiveKind,
    line: usize,
    rules: Vec<String>,
}

pub fn suppress_diagnostics<'tree>(
    root: &Node<'tree>,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    let directives = extract_directives(root);
    apply_directives(diagnostics, &directives)
}

fn extract_directives<'tree>(root: &Node<'tree>) -> Vec<LintDirective> {
    let file_head_end_byte = file_head_end_byte(root);

    root.descendants()
        .filter(|node| node.child_count() == 0 && node.kind() == SyntaxKind::SQL_COMMENT)
        .filter_map(|comment| {
            let directive = parse_directive(comment.text(), comment.range().start_position.row)?;

            if directive.kind == LintDirectiveKind::Disable
                && comment.range().start_byte >= file_head_end_byte
            {
                return None;
            }

            Some(directive)
        })
        .collect()
}

fn file_head_end_byte<'tree>(root: &Node<'tree>) -> usize {
    root.descendants()
        .filter(|node| node.child_count() == 0)
        .find_map(|node| match node.kind() {
            SyntaxKind::SQL_COMMENT => None,
            SyntaxKind::C_COMMENT => Some(node.range().start_byte),
            _ => Some(node.range().start_byte),
        })
        .unwrap_or(usize::MAX)
}

fn parse_directive(comment_text: &str, line: usize) -> Option<LintDirective> {
    const DISABLE_NEXT_LINE: &str = "uroborosql-lint-disable-next-line";
    const DISABLE: &str = "uroborosql-lint-disable";

    let body = comment_text.strip_prefix("--")?;
    let body = body.trim_start_matches([' ', '\t']);

    if let Some(rest) = body.strip_prefix(DISABLE_NEXT_LINE) {
        return parse_rules(rest).map(|rules| LintDirective {
            kind: LintDirectiveKind::DisableNextLine,
            line,
            rules,
        });
    }

    if let Some(rest) = body.strip_prefix(DISABLE) {
        return parse_rules(rest).map(|rules| LintDirective {
            kind: LintDirectiveKind::Disable,
            line,
            rules,
        });
    }

    None
}

fn parse_rules(rest: &str) -> Option<Vec<String>> {
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let mut seen = HashSet::new();
    let mut rules = Vec::new();
    let rest = rest.trim();

    if rest.is_empty() {
        return None;
    }

    for name in rest.split(',').map(str::trim) {
        if name.is_empty() || RuleEnum::from_name(name).is_none() {
            return None;
        }

        if seen.insert(name) {
            rules.push(name.to_string());
        }
    }

    Some(rules)
}

fn apply_directives(diagnostics: Vec<Diagnostic>, directives: &[LintDirective]) -> Vec<Diagnostic> {
    let mut file_disabled_rules = HashSet::new();
    let mut next_line_disabled_rules: HashMap<usize, HashSet<String>> = HashMap::new();

    for directive in directives {
        match directive.kind {
            LintDirectiveKind::Disable => {
                file_disabled_rules.extend(directive.rules.iter().cloned());
            }
            LintDirectiveKind::DisableNextLine => {
                next_line_disabled_rules
                    .entry(directive.line + 1)
                    .or_default()
                    .extend(directive.rules.iter().cloned());
            }
        }
    }

    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            !file_disabled_rules.contains(diagnostic.rule_id)
                && !next_line_disabled_rules
                    .get(&diagnostic.span.start.line)
                    .is_some_and(|rules| rules.contains(diagnostic.rule_id))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diagnostic::Severity,
        linter::tests::run_with_rules,
        rules::{NoDistinct, NoWildcardProjection, RuleEnum},
        Linter, ResolvedLintConfig,
    };
    use postgresql_cst_parser::tree_sitter;

    fn parse(sql: &str) -> postgresql_cst_parser::tree_sitter::Tree {
        tree_sitter::parse_2way(sql).expect("parse ok")
    }

    fn parse_comment_directive(comment: &str) -> Option<LintDirective> {
        parse_directive(comment, 3)
    }

    #[test]
    fn parses_directive_with_optional_space_after_prefix() {
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct"),
            Some(LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 3,
                rules: vec!["no-distinct".to_string()],
            })
        );
        assert_eq!(
            parse_comment_directive("--uroborosql-lint-disable-next-line no-distinct"),
            Some(LintDirective {
                kind: LintDirectiveKind::DisableNextLine,
                line: 3,
                rules: vec!["no-distinct".to_string()],
            })
        );
    }

    #[test]
    fn parses_multiple_rules_and_deduplicates_them() {
        assert_eq!(
            parse_comment_directive(
                "-- uroborosql-lint-disable no-distinct, no-wildcard-projection, no-distinct"
            ),
            Some(LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 3,
                rules: vec![
                    "no-distinct".to_string(),
                    "no-wildcard-projection".to_string(),
                ],
            })
        );
    }

    #[test]
    fn rejects_invalid_directive_inputs() {
        assert_eq!(parse_comment_directive("-- uroborosql-lint-disable"), None);
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct,"),
            None
        );
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable unknown-rule"),
            None
        );
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct because reason"),
            None
        );
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
        assert_eq!(diagnostics[0].rule_id, "no-wildcard-projection");
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
        let sql = r#"-- uroborosql-lint-disable no-distinct
SELECT DISTINCT id FROM users;"#;
        let resolved_config = ResolvedLintConfig {
            rules: vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)],
            db: None,
        };

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn extracts_only_head_disable_directives() {
        let sql = r#"-- uroborosql-lint-disable no-distinct
SELECT 1;
-- uroborosql-lint-disable no-wildcard-projection"#;
        let tree = parse(sql);

        let directives = extract_directives(&tree.root_node());

        assert_eq!(
            directives,
            vec![LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 0,
                rules: vec!["no-distinct".to_string()],
            }]
        );
    }
}
