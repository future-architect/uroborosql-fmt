use std::collections::{HashMap, HashSet};

use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Point, Range},
};

use crate::{
    diagnostic::{Diagnostic, Severity},
    rules::RuleEnum,
};

const INVALID_LINT_DIRECTIVE_CODE: &str = "invalid-lint-directive";

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
    let (directives, mut directive_diagnostics) = extract_directives(root);
    let mut diagnostics = apply_directives(diagnostics, &directives);
    diagnostics.append(&mut directive_diagnostics);
    diagnostics.sort_by_key(|diag| (diag.span.start.byte, diag.span.end.byte, diag.code));
    diagnostics
}

fn extract_directives<'tree>(root: &Node<'tree>) -> (Vec<LintDirective>, Vec<Diagnostic>) {
    let file_head_end_byte = file_head_end_byte(root);
    let mut directives = Vec::new();
    let mut diagnostics = Vec::new();

    for comment in root
        .descendants()
        .filter(|node| node.child_count() == 0 && node.kind() == SyntaxKind::SQL_COMMENT)
    {
        match parse_directive(&comment) {
            ParsedDirective::NotDirective => {}
            ParsedDirective::Invalid(diagnostic) => diagnostics.push(diagnostic),
            ParsedDirective::Valid(directive) => {
                if directive.kind == LintDirectiveKind::Disable
                    && comment.range().start_byte >= file_head_end_byte
                {
                    continue;
                }

                directives.push(directive);
            }
        }
    }

    (directives, diagnostics)
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

#[derive(Debug, PartialEq)]
enum ParsedDirective {
    NotDirective,
    Invalid(Diagnostic),
    Valid(LintDirective),
}

fn parse_directive<'tree>(comment: &Node<'tree>) -> ParsedDirective {
    const DISABLE_NEXT_LINE: &str = "uroborosql-lint-disable-next-line";
    const DISABLE: &str = "uroborosql-lint-disable";

    let comment_text = comment.text();
    let line = comment.range().start_position.row;
    let Some(body) = comment_text.strip_prefix("--") else {
        return ParsedDirective::NotDirective;
    };
    let body = body.trim_start_matches([' ', '\t']);
    let body_offset = comment_text.len() - body.len();

    if let Some(rest) = body.strip_prefix(DISABLE_NEXT_LINE) {
        return parse_rules(comment, rest, body_offset + DISABLE_NEXT_LINE.len()).map_or(
            ParsedDirective::NotDirective,
            |parsed| match parsed {
                ParsedRules::Valid(rules) => ParsedDirective::Valid(LintDirective {
                    kind: LintDirectiveKind::DisableNextLine,
                    line,
                    rules,
                }),
                ParsedRules::Invalid(diagnostic) => ParsedDirective::Invalid(diagnostic),
                ParsedRules::Ignore => ParsedDirective::NotDirective,
            },
        );
    }

    if let Some(rest) = body.strip_prefix(DISABLE) {
        return parse_rules(comment, rest, body_offset + DISABLE.len()).map_or(
            ParsedDirective::NotDirective,
            |parsed| match parsed {
                ParsedRules::Valid(rules) => ParsedDirective::Valid(LintDirective {
                    kind: LintDirectiveKind::Disable,
                    line,
                    rules,
                }),
                ParsedRules::Invalid(diagnostic) => ParsedDirective::Invalid(diagnostic),
                ParsedRules::Ignore => ParsedDirective::NotDirective,
            },
        );
    }

    ParsedDirective::NotDirective
}

enum ParsedRules {
    Valid(Vec<String>),
    Invalid(Diagnostic),
    Ignore,
}

fn parse_rules<'tree>(
    comment: &Node<'tree>,
    rest: &str,
    directive_offset: usize,
) -> Option<ParsedRules> {
    if !rest.starts_with(char::is_whitespace) {
        return Some(ParsedRules::Ignore);
    }

    let mut seen = HashSet::new();
    let mut rules = Vec::new();
    let rules_offset = directive_offset + (rest.len() - rest.trim_start().len());
    let rest = rest.trim();

    if rest.is_empty() {
        return Some(ParsedRules::Ignore);
    }

    let mut offset = 0;
    for raw_name in rest.split(',') {
        let trimmed_start = raw_name.len() - raw_name.trim_start().len();
        let trimmed_end = raw_name.trim_end().len();
        let name = raw_name[trimmed_start..trimmed_end].trim();

        if name.is_empty() {
            return Some(ParsedRules::Ignore);
        }

        if name.contains(char::is_whitespace) {
            return Some(ParsedRules::Ignore);
        }

        if RuleEnum::from_name(name).is_none() {
            let start = rules_offset + offset + trimmed_start;
            let end = start + name.len();
            return Some(ParsedRules::Invalid(unknown_rule_diagnostic(
                comment, name, start, end,
            )));
        }

        if seen.insert(name) {
            rules.push(name.to_string());
        }

        offset += raw_name.len() + 1;
    }

    Some(ParsedRules::Valid(rules))
}

fn unknown_rule_diagnostic<'tree>(
    comment: &Node<'tree>,
    unknown_rule: &str,
    start_offset: usize,
    end_offset: usize,
) -> Diagnostic {
    let range = subrange_in_comment(comment, start_offset, end_offset);
    Diagnostic::new(
        INVALID_LINT_DIRECTIVE_CODE,
        Severity::Warning,
        format!("unknown lint directive rule `{unknown_rule}`"),
        &range,
    )
}

fn subrange_in_comment<'tree>(
    comment: &Node<'tree>,
    start_offset: usize,
    end_offset: usize,
) -> Range {
    let full_range = comment.range();
    Range {
        start_byte: full_range.start_byte + start_offset,
        end_byte: full_range.start_byte + end_offset,
        start_position: Point {
            row: full_range.start_position.row,
            column: full_range.start_position.column + start_offset,
        },
        end_position: Point {
            row: full_range.start_position.row,
            column: full_range.start_position.column + end_offset,
        },
    }
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
            !file_disabled_rules.contains(diagnostic.code)
                && !next_line_disabled_rules
                    .get(&diagnostic.span.start.line)
                    .is_some_and(|rules| rules.contains(diagnostic.code))
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

    fn parse_comment_directive(comment: &str) -> ParsedDirective {
        let tree = parse(comment);
        let comment_node = tree
            .root_node()
            .descendants()
            .find(|node| node.kind() == SyntaxKind::SQL_COMMENT)
            .expect("sql comment");
        parse_directive(&comment_node)
    }

    #[test]
    fn parses_directive_with_optional_space_after_prefix() {
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct"),
            ParsedDirective::Valid(LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 0,
                rules: vec!["no-distinct".to_string()],
            })
        );
        assert_eq!(
            parse_comment_directive("--uroborosql-lint-disable-next-line no-distinct"),
            ParsedDirective::Valid(LintDirective {
                kind: LintDirectiveKind::DisableNextLine,
                line: 0,
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
            ParsedDirective::Valid(LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 0,
                rules: vec![
                    "no-distinct".to_string(),
                    "no-wildcard-projection".to_string(),
                ],
            })
        );
    }

    #[test]
    fn rejects_invalid_directive_inputs() {
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable"),
            ParsedDirective::NotDirective
        ));
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct,"),
            ParsedDirective::NotDirective
        );
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable unknown-rule"),
            ParsedDirective::Invalid(_)
        ));
        assert_eq!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct because reason"),
            ParsedDirective::NotDirective
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

        let (directives, diagnostics) = extract_directives(&tree.root_node());

        assert!(diagnostics.is_empty());
        assert_eq!(
            directives,
            vec![LintDirective {
                kind: LintDirectiveKind::Disable,
                line: 0,
                rules: vec!["no-distinct".to_string()],
            }]
        );
    }

    #[test]
    fn unknown_rule_in_directive_produces_warning_on_rule_name() {
        let sql = r#"-- uroborosql-lint-disable no-dstinct
SELECT DISTINCT id FROM users;"#;
        let resolved_config = ResolvedLintConfig {
            rules: vec![(RuleEnum::NoDistinct(NoDistinct), Severity::Warning)],
            db: None,
        };

        let diagnostics = Linter::new().run(sql, &resolved_config).expect("lint ok");

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code, INVALID_LINT_DIRECTIVE_CODE);
        assert_eq!(
            diagnostics[0].message,
            "unknown lint directive rule `no-dstinct`"
        );
        assert_eq!(diagnostics[0].span.start.line, 0);
        assert_eq!(diagnostics[0].span.start.column, 27);
        assert_eq!(diagnostics[0].span.end.column, 37);
        assert_eq!(diagnostics[1].code, "no-distinct");
    }
}
