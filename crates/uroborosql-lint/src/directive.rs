use std::collections::{HashMap, HashSet};

use postgresql_cst_parser::{
    syntax_kind::SyntaxKind,
    tree_sitter::{Node, Point, Range},
};

use crate::{
    diagnostic::{Diagnostic, Severity},
    rules::RuleEnum,
};

pub const LINT_SOURCE: &str = "uroborosql-lint";
pub const INVALID_LINT_DIRECTIVE_CODE: &str = "invalid-lint-directive";
pub const DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD: &str = "uroborosql-lint-disable-next-line";
pub const DISABLE_DIRECTIVE_KEYWORD: &str = "uroborosql-lint-disable";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParsedLintDirectiveKind {
    Disable,
    DisableNextLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectiveParseDiagnosticKind {
    UnknownRule {
        rule: String,
        removal_range: UnknownRuleRemovalRange,
    },
    SyntaxError {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownRuleRemovalRange {
    FullLine,
    PartialLine(std::ops::Range<usize>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectiveParseDiagnostic {
    pub kind: DirectiveParseDiagnosticKind,
    pub span: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLineComment {
    NotLintDirective,
    LintDirective {
        kind: ParsedLintDirectiveKind,
        rules: Vec<String>,
        diagnostics: Vec<DirectiveParseDiagnostic>,
        append_byte: Option<usize>,
        has_syntax_error: bool,
    },
}

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
    let comment_text = comment.text();
    let line = comment.range().start_position.row;

    match parse_line_comment_directive(comment_text) {
        ParsedLineComment::NotLintDirective => ParsedDirective::NotDirective,
        ParsedLineComment::LintDirective {
            kind,
            rules,
            diagnostics,
            has_syntax_error,
            ..
        } => {
            let diagnostics = diagnostics
                .into_iter()
                .map(|diagnostic| to_lint_diagnostic(comment, diagnostic))
                .collect::<Vec<_>>();
            if has_syntax_error {
                ParsedDirective::Invalid(
                    diagnostics
                        .into_iter()
                        .next()
                        .expect("syntax diagnostic should be present"),
                )
            } else {
                ParsedDirective::Valid {
                    directive: LintDirective {
                        kind: match kind {
                            ParsedLintDirectiveKind::Disable => LintDirectiveKind::Disable,
                            ParsedLintDirectiveKind::DisableNextLine => {
                                LintDirectiveKind::DisableNextLine
                            }
                        },
                        line,
                        rules,
                    },
                    diagnostics,
                }
            }
        }
    }
}

pub fn parse_line_comment_directive(text: &str) -> ParsedLineComment {
    let Some(body) = text.strip_prefix("--") else {
        return ParsedLineComment::NotLintDirective;
    };
    let body = body.trim_start_matches([' ', '\t']);
    let body_offset = text.len() - body.len();

    if let Some(rest) = body.strip_prefix(DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD) {
        return parse_line_rules(
            text,
            ParsedLintDirectiveKind::DisableNextLine,
            rest,
            body_offset + DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD.len(),
        );
    }

    if let Some(rest) = body.strip_prefix(DISABLE_DIRECTIVE_KEYWORD) {
        return parse_line_rules(
            text,
            ParsedLintDirectiveKind::Disable,
            rest,
            body_offset + DISABLE_DIRECTIVE_KEYWORD.len(),
        );
    }

    ParsedLineComment::NotLintDirective
}

fn parse_line_rules(
    text: &str,
    kind: ParsedLintDirectiveKind,
    rest: &str,
    directive_offset: usize,
) -> ParsedLineComment {
    if rest.trim().is_empty() {
        return syntax_error_directive(
            kind,
            "invalid lint directive syntax: expected one or more comma-separated rule names",
            0,
            text.len(),
        );
    }

    if !rest.starts_with(char::is_whitespace) {
        return ParsedLineComment::NotLintDirective;
    }

    let rules_offset = directive_offset + (rest.len() - rest.trim_start().len());
    let rest = rest.trim_start().trim_end();
    let mut seen = HashSet::new();
    let mut rules = Vec::new();
    let mut diagnostics = Vec::new();
    let segments = split_rule_segments(rest);

    for (index, segment) in segments.iter().enumerate() {
        let raw_name = &rest[segment.start..segment.end];
        let name = raw_name.trim_matches([' ', '\t']);
        let trimmed_start = raw_name.len() - raw_name.trim_start_matches([' ', '\t']).len();

        if name.is_empty() {
            let start = rules_offset + segment.start + trimmed_start;
            let end = if segment.end == rest.len() {
                start.saturating_sub(1)
            } else {
                start + raw_name.len()
            };
            let message = if segment.end == rest.len() {
                "invalid lint directive syntax: trailing comma is not allowed"
            } else {
                "invalid lint directive syntax: expected comma-separated rule names"
            };
            return syntax_error_directive(kind, message, start.saturating_sub(1), end);
        }

        if RuleEnum::from_name(name).is_none() {
            let start = rules_offset + segment.start + trimmed_start;
            let end = start + name.len();
            let removal_range = unknown_rule_removal_range(rest, &segments, index);
            diagnostics.push(DirectiveParseDiagnostic {
                kind: DirectiveParseDiagnosticKind::UnknownRule {
                    rule: name.to_string(),
                    removal_range: match removal_range {
                        UnknownRuleRemovalRange::FullLine => UnknownRuleRemovalRange::FullLine,
                        UnknownRuleRemovalRange::PartialLine(range) => {
                            UnknownRuleRemovalRange::PartialLine(
                                range.start + rules_offset..range.end + rules_offset,
                            )
                        }
                    },
                },
                span: start..end,
            });
            continue;
        }

        if seen.insert(name) {
            rules.push(name.to_string());
        }
    }

    ParsedLineComment::LintDirective {
        kind,
        rules,
        diagnostics,
        append_byte: Some(rules_offset + rest.len()),
        has_syntax_error: false,
    }
}

#[derive(Debug)]
struct RuleSegment {
    start: usize,
    end: usize,
}

fn split_rule_segments(rest: &str) -> Vec<RuleSegment> {
    let mut segments = Vec::new();
    let mut start = 0;
    for (idx, _) in rest.match_indices(',') {
        segments.push(RuleSegment { start, end: idx });
        start = idx + 1;
    }
    segments.push(RuleSegment {
        start,
        end: rest.len(),
    });
    segments
}

fn unknown_rule_removal_range(
    rest: &str,
    segments: &[RuleSegment],
    index: usize,
) -> UnknownRuleRemovalRange {
    if segments.len() == 1 {
        return UnknownRuleRemovalRange::FullLine;
    }

    let segment = &segments[index];
    if index + 1 < segments.len() {
        let raw_name = &rest[segment.start..segment.end];
        let trimmed_start = raw_name.len() - raw_name.trim_start_matches([' ', '\t']).len();
        let next = &segments[index + 1];
        let next_raw = &rest[next.start..next.end];
        let next_trimmed_start = next_raw.len() - next_raw.trim_start_matches([' ', '\t']).len();
        UnknownRuleRemovalRange::PartialLine(
            segment.start + trimmed_start..next.start + next_trimmed_start,
        )
    } else {
        let previous = &segments[index - 1];
        let previous_raw = &rest[previous.start..previous.end];
        let previous_trimmed_end = previous_raw.trim_end_matches([' ', '\t']).len();
        UnknownRuleRemovalRange::PartialLine(previous.start + previous_trimmed_end..segment.end)
    }
}

fn syntax_error_directive(
    kind: ParsedLintDirectiveKind,
    message: impl Into<String>,
    start_offset: usize,
    end_offset: usize,
) -> ParsedLineComment {
    ParsedLineComment::LintDirective {
        kind,
        rules: Vec::new(),
        diagnostics: vec![DirectiveParseDiagnostic {
            kind: DirectiveParseDiagnosticKind::SyntaxError {
                message: message.into(),
            },
            span: start_offset..end_offset.max(start_offset + 1),
        }],
        append_byte: None,
        has_syntax_error: true,
    }
}

fn to_lint_diagnostic<'tree>(
    comment: &Node<'tree>,
    diagnostic: DirectiveParseDiagnostic,
) -> Diagnostic {
    match diagnostic.kind {
        DirectiveParseDiagnosticKind::UnknownRule { rule, .. } => {
            let range = subrange_in_comment(comment, diagnostic.span.start, diagnostic.span.end);
            Diagnostic::new(
                INVALID_LINT_DIRECTIVE_CODE,
                Severity::Warning,
                format!("unknown lint directive rule `{rule}`"),
                &range,
            )
        }
        DirectiveParseDiagnosticKind::SyntaxError { message } => {
            let range = subrange_in_comment(comment, diagnostic.span.start, diagnostic.span.end);
            Diagnostic::new(
                INVALID_LINT_DIRECTIVE_CODE,
                Severity::Warning,
                message,
                &range,
            )
        }
    }
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
    fn line_parser_returns_known_rules_and_unknown_diagnostics() {
        let parsed = parse_line_comment_directive(
            "-- uroborosql-lint-disable no-distinct, clearly-not-a-rule",
        );

        assert!(matches!(
            parsed,
            ParsedLineComment::LintDirective {
                kind: ParsedLintDirectiveKind::Disable,
                rules,
                diagnostics,
                append_byte: Some(58),
                has_syntax_error: false,
            } if rules == vec!["no-distinct".to_string()]
                && matches!(
                    &diagnostics[..],
                    [DirectiveParseDiagnostic {
                        kind: DirectiveParseDiagnosticKind::UnknownRule { rule, removal_range },
                        span,
                    }] if rule == "clearly-not-a-rule"
                        && removal_range
                            == &UnknownRuleRemovalRange::PartialLine(38..58)
                        && span == &(40..58)
                )
        ));
    }

    #[test]
    fn line_parser_reports_syntax_error_without_append_position() {
        let parsed = parse_line_comment_directive("-- uroborosql-lint-disable no-distinct,");

        assert!(matches!(
            parsed,
            ParsedLineComment::LintDirective {
                has_syntax_error: true,
                append_byte: None,
                diagnostics,
                ..
            } if matches!(
                &diagnostics[..],
                [DirectiveParseDiagnostic {
                    kind: DirectiveParseDiagnosticKind::SyntaxError { message },
                    ..
                }] if message == "invalid lint directive syntax: trailing comma is not allowed"
            )
        ));
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
