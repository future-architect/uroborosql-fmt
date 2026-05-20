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
    let file_head_comment_end_byte = file_head_comment_end_byte(root);
    let mut directives = Vec::new();
    let mut diagnostics = Vec::new();

    for comment in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::SQL_COMMENT)
    {
        match parse_directive(&comment) {
            ParsedDirective::NotDirective => {}
            ParsedDirective::Invalid(diagnostic) => diagnostics.push(diagnostic),
            ParsedDirective::Valid {
                directive,
                diagnostics: mut directive_diagnostics,
            } => {
                diagnostics.append(&mut directive_diagnostics);
                if directive.kind == LintDirectiveKind::Disable
                    && comment.range().start_byte >= file_head_comment_end_byte
                {
                    continue;
                }

                directives.push(directive);
            }
        }
    }

    (directives, diagnostics)
}

/// Returns the byte offset where the leading line-comment section ends.
///
/// File-head `disable` directives are valid only inside the initial run of
/// blank lines and SQL line comments. The first block comment or other token
/// ends that section.
fn file_head_comment_end_byte<'tree>(root: &Node<'tree>) -> usize {
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
    Valid {
        directive: LintDirective,
        diagnostics: Vec<Diagnostic>,
    },
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
        return match parse_rules(comment, rest, body_offset + DISABLE_NEXT_LINE.len()) {
            ParsedRules::Valid { rules, diagnostics } => ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::DisableNextLine,
                    line,
                    rules,
                },
                diagnostics,
            },
            ParsedRules::Invalid(diagnostic) => ParsedDirective::Invalid(diagnostic),
            ParsedRules::NotDirective => ParsedDirective::NotDirective,
        };
    }

    if let Some(rest) = body.strip_prefix(DISABLE) {
        return match parse_rules(comment, rest, body_offset + DISABLE.len()) {
            ParsedRules::Valid { rules, diagnostics } => ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::Disable,
                    line,
                    rules,
                },
                diagnostics,
            },
            ParsedRules::Invalid(diagnostic) => ParsedDirective::Invalid(diagnostic),
            ParsedRules::NotDirective => ParsedDirective::NotDirective,
        };
    }

    ParsedDirective::NotDirective
}

enum ParsedRules {
    Valid {
        rules: Vec<String>,
        diagnostics: Vec<Diagnostic>,
    },
    Invalid(Diagnostic),
    NotDirective,
}

fn parse_rules<'tree>(comment: &Node<'tree>, rest: &str, directive_offset: usize) -> ParsedRules {
    if rest.trim().is_empty() {
        return ParsedRules::Invalid(invalid_syntax_diagnostic(
            comment,
            "invalid lint directive syntax: expected one or more comma-separated rule names",
            0,
            comment.text().len(),
        ));
    }

    if !rest.starts_with(char::is_whitespace) {
        return ParsedRules::NotDirective;
    }

    // Keep spans in original comment coordinates for directive diagnostics.
    let rules_offset = directive_offset + (rest.len() - rest.trim_start().len());
    let rest = rest.trim();
    let mut seen = HashSet::new();
    let mut rules = Vec::new();
    let mut diagnostics = Vec::new();

    let mut offset = 0;
    for raw_name in rest.split(',') {
        let name = raw_name.trim();
        let trimmed_start = raw_name.len() - raw_name.trim_start().len();

        if name.is_empty() {
            let start = rules_offset + offset + trimmed_start;
            let end = if offset + raw_name.len() == rest.len() {
                start.saturating_sub(1)
            } else {
                start + raw_name.len()
            };
            let message = if offset + raw_name.len() == rest.len() {
                "invalid lint directive syntax: trailing comma is not allowed"
            } else {
                "invalid lint directive syntax: expected comma-separated rule names"
            };
            return ParsedRules::Invalid(invalid_syntax_diagnostic(
                comment,
                message,
                start.saturating_sub(1),
                end,
            ));
        }

        if RuleEnum::from_name(name).is_none() {
            let start = rules_offset + offset + trimmed_start;
            let end = start + name.len();
            diagnostics.push(unknown_rule_diagnostic(comment, name, start, end));
            offset += raw_name.len() + 1;
            continue;
        }

        if seen.insert(name) {
            rules.push(name.to_string());
        }

        offset += raw_name.len() + 1;
    }

    ParsedRules::Valid { rules, diagnostics }
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

fn invalid_syntax_diagnostic<'tree>(
    comment: &Node<'tree>,
    message: impl Into<String>,
    start_offset: usize,
    end_offset: usize,
) -> Diagnostic {
    let range = subrange_in_comment(comment, start_offset, end_offset.max(start_offset + 1));
    Diagnostic::new(
        INVALID_LINT_DIRECTIVE_CODE,
        Severity::Warning,
        message,
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
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct"),
            ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::Disable,
                    line: 0,
                    rules,
                },
                diagnostics,
            } if rules == vec!["no-distinct".to_string()] && diagnostics.is_empty()
        ));
        assert!(matches!(
            parse_comment_directive("--uroborosql-lint-disable-next-line no-distinct"),
            ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::DisableNextLine,
                    line: 0,
                    rules,
                },
                diagnostics,
            } if rules == vec!["no-distinct".to_string()] && diagnostics.is_empty()
        ));
    }

    #[test]
    fn parses_multiple_rules_and_deduplicates_them() {
        assert!(matches!(
            parse_comment_directive(
                "-- uroborosql-lint-disable no-distinct, no-wildcard-projection, no-distinct"
            ),
            ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::Disable,
                    line: 0,
                    rules,
                },
                diagnostics,
            } if rules == vec![
                "no-distinct".to_string(),
                "no-wildcard-projection".to_string(),
            ] && diagnostics.is_empty()
        ));
    }

    #[test]
    fn rejects_invalid_directive_inputs() {
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable"),
            ParsedDirective::Invalid(_)
        ));
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct,"),
            ParsedDirective::Invalid(_)
        ));
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable  , , "),
            ParsedDirective::Invalid(_)
        ));
    }

    #[test]
    fn keeps_known_rules_when_unknown_rules_are_mixed_in() {
        assert!(matches!(
            parse_comment_directive("-- uroborosql-lint-disable no-distinct, clearly-not-a-rule"),
            ParsedDirective::Valid {
                directive: LintDirective {
                    kind: LintDirectiveKind::Disable,
                    line: 0,
                    rules,
                },
                diagnostics,
            } if rules == vec!["no-distinct".to_string()]
                && diagnostics.len() == 1
                && diagnostics[0].code == INVALID_LINT_DIRECTIVE_CODE
                && diagnostics[0].message == "unknown lint directive rule `clearly-not-a-rule`"
        ));
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
}
