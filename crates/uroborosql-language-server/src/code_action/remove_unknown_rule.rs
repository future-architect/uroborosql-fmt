use ropey::Rope;
use tower_lsp_server::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, TextEdit, Uri,
};
use uroborosql_lint::{
    DirectiveParseDiagnosticKind, ParsedLineComment, UnknownRuleRemovalRange,
    parse_line_comment_directive,
};

use super::{INVALID_LINT_DIRECTIVE_CODE, LINT_SOURCE, directive_line, workspace_edit};
use crate::document::{
    rope_line_byte_range_to_range, rope_line_has_ending, rope_line_text_without_ending,
};

pub(in crate::code_action) fn remove_unknown_rule_action(
    uri: &Uri,
    rope: &Rope,
    diagnostic: &Diagnostic,
) -> Option<CodeAction> {
    if diagnostic.source.as_deref() != Some(LINT_SOURCE) {
        return None;
    }
    if !matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == INVALID_LINT_DIRECTIVE_CODE
    ) {
        return None;
    }

    let diagnostic_line = diagnostic.range.start.line;
    let line_text = rope_line_text_without_ending(rope, diagnostic_line)?;
    let (directive_text, directive_offset) = directive_line::directive_text_with_offset(&line_text);
    let ParsedLineComment::LintDirective { diagnostics, .. } =
        parse_line_comment_directive(directive_text)
    else {
        return None;
    };

    for parse_diagnostic in diagnostics {
        let DirectiveParseDiagnosticKind::UnknownRule { removal_range, .. } = parse_diagnostic.kind
        else {
            continue;
        };
        let span_range = rope_line_byte_range_to_range(
            rope,
            diagnostic_line,
            &line_text,
            directive_line::add_offset(parse_diagnostic.span, directive_offset),
        )?;
        if span_range != diagnostic.range {
            continue;
        }

        let removal_range = line_removal_range_with_offset(removal_range, directive_offset);
        let edit_range =
            removal_range_to_lsp_range(rope, diagnostic_line, &line_text, removal_range)?;
        return Some(CodeAction {
            title: "Remove unknown lint rule".into(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: Some(vec![diagnostic.clone()]),
            edit: Some(workspace_edit(
                uri,
                TextEdit {
                    range: edit_range,
                    new_text: String::new(),
                },
            )),
            ..CodeAction::default()
        });
    }

    None
}

fn line_removal_range_with_offset(
    removal_range: UnknownRuleRemovalRange,
    directive_offset: usize,
) -> UnknownRuleRemovalRange {
    match removal_range {
        UnknownRuleRemovalRange::FullLine => UnknownRuleRemovalRange::FullLine,
        UnknownRuleRemovalRange::PartialLine(range) => UnknownRuleRemovalRange::PartialLine(
            directive_line::add_offset(range, directive_offset),
        ),
    }
}

fn removal_range_to_lsp_range(
    rope: &Rope,
    line: u32,
    line_text: &str,
    removal_range: UnknownRuleRemovalRange,
) -> Option<Range> {
    match removal_range {
        UnknownRuleRemovalRange::FullLine if rope_line_has_ending(rope, line) => Some(Range::new(
            Position::new(line, 0),
            Position::new(line + 1, 0),
        )),
        UnknownRuleRemovalRange::FullLine => {
            rope_line_byte_range_to_range(rope, line, line_text, 0..line_text.len())
        }
        UnknownRuleRemovalRange::PartialLine(range) => {
            rope_line_byte_range_to_range(rope, line, line_text, range)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn remove_unknown_rule_whole_line_includes_line_ending() {
        let rope = rope("-- uroborosql-lint-disable-next-line definitely-not-a-rule\nSELECT 1;\n");
        let range = removal_range_to_lsp_range(
            &rope,
            0,
            "-- uroborosql-lint-disable-next-line definitely-not-a-rule",
            UnknownRuleRemovalRange::FullLine,
        )
        .unwrap();

        assert_eq!(range, Range::new(Position::new(0, 0), Position::new(1, 0)));
    }
}
