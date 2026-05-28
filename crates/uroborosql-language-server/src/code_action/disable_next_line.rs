use std::collections::HashSet;

use ropey::Rope;
use tower_lsp_server::lsp_types::{
    CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, TextEdit, Uri,
    WorkspaceEdit,
};
use uroborosql_lint::{
    DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD, ParsedLineComment, ParsedLintDirectiveKind, RuleEnum,
    parse_line_comment_directive,
};

use super::{INVALID_LINT_DIRECTIVE_CODE, LINT_SOURCE, directive_line, workspace_edit};
use crate::document::{
    rope_line_byte_to_position, rope_line_exists, rope_line_text_without_ending,
};

pub(in crate::code_action) fn disable_next_line_action(
    uri: &Uri,
    rope: &Rope,
    diagnostic: &Diagnostic,
    seen: &mut HashSet<(u32, String)>,
) -> Option<CodeAction> {
    if diagnostic.source.as_deref() != Some(LINT_SOURCE) {
        return None;
    }

    let rule_name = match diagnostic.code.as_ref()? {
        NumberOrString::String(rule) if rule != INVALID_LINT_DIRECTIVE_CODE => rule,
        _ => return None,
    };
    let _ = RuleEnum::from_name(rule_name)?;

    let target_line = diagnostic.range.start.line;
    if !rope_line_exists(rope, target_line) || !seen.insert((target_line, rule_name.clone())) {
        return None;
    }

    let edit = build_disable_next_line_directive_edit(uri, rope, target_line, rule_name)?;

    Some(CodeAction {
        title: format!("Disable {rule_name} for next line"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(edit),
        ..CodeAction::default()
    })
}

fn build_disable_next_line_directive_edit(
    uri: &Uri,
    rope: &Rope,
    target_line: u32,
    rule: &str,
) -> Option<WorkspaceEdit> {
    if target_line == 0 {
        return build_insert_disable_next_line_directive_edit(uri, rope, target_line, rule);
    }

    match find_existing_disable_next_line_directive(rope, target_line - 1, rule)? {
        ExistingDisableNextLineDirective::Append(position) => Some(workspace_edit(
            uri,
            TextEdit {
                range: Range::new(position, position),
                new_text: format!(", {rule}"),
            },
        )),
        ExistingDisableNextLineDirective::AlreadyContains
        | ExistingDisableNextLineDirective::Blocked => None,
        ExistingDisableNextLineDirective::NotDirective => {
            build_insert_disable_next_line_directive_edit(uri, rope, target_line, rule)
        }
    }
}

enum ExistingDisableNextLineDirective {
    Append(Position),
    AlreadyContains,
    Blocked,
    NotDirective,
}

fn find_existing_disable_next_line_directive(
    rope: &Rope,
    directive_line: u32,
    rule: &str,
) -> Option<ExistingDisableNextLineDirective> {
    let line_text = rope_line_text_without_ending(rope, directive_line)?;
    let (directive_text, directive_offset) = directive_line::directive_text_with_offset(&line_text);
    match parse_line_comment_directive(directive_text) {
        ParsedLineComment::LintDirective {
            kind: ParsedLintDirectiveKind::DisableNextLine,
            rules,
            append_byte,
            has_syntax_error,
            ..
        } => {
            if rules.iter().any(|existing| existing == rule) {
                return Some(ExistingDisableNextLineDirective::AlreadyContains);
            }
            if has_syntax_error {
                return Some(ExistingDisableNextLineDirective::Blocked);
            }
            let append_byte = append_byte?;
            let position = rope_line_byte_to_position(
                rope,
                directive_line,
                &line_text,
                directive_offset + append_byte,
            )?;
            Some(ExistingDisableNextLineDirective::Append(position))
        }
        ParsedLineComment::LintDirective {
            kind: ParsedLintDirectiveKind::Disable,
            ..
        }
        | ParsedLineComment::NotLintDirective => {
            Some(ExistingDisableNextLineDirective::NotDirective)
        }
    }
}

fn build_insert_disable_next_line_directive_edit(
    uri: &Uri,
    rope: &Rope,
    target_line: u32,
    rule: &str,
) -> Option<WorkspaceEdit> {
    let line_text = rope_line_text_without_ending(rope, target_line)?;
    let indent = directive_line::leading_whitespace(&line_text);
    let line_ending = detect_line_ending(rope, target_line);
    let position = Position::new(target_line, 0);
    Some(workspace_edit(
        uri,
        TextEdit {
            range: Range::new(position, position),
            new_text: format!(
                "{indent}-- {DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD} {rule}{line_ending}"
            ),
        },
    ))
}

fn detect_line_ending(rope: &Rope, target_line: u32) -> &'static str {
    if let Some(line_ending) = line_ending_at(rope, target_line) {
        return line_ending;
    }

    (0..rope.len_lines())
        .find_map(|line| line_ending_at(rope, line as u32))
        .unwrap_or("\n")
}

fn line_ending_at(rope: &Rope, line: u32) -> Option<&'static str> {
    if !rope_line_exists(rope, line) {
        return None;
    }

    let line_text = rope.line(line as usize).to_string();
    if line_text.ends_with("\r\n") {
        Some("\r\n")
    } else if line_text.ends_with('\n') {
        Some("\n")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope(text: &str) -> Rope {
        Rope::from_str(text)
    }

    fn test_uri() -> Uri {
        "file:///test.sql".parse().unwrap()
    }

    fn only_edit(edit: WorkspaceEdit) -> TextEdit {
        edit.changes
            .unwrap()
            .into_values()
            .next()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn build_insert_disable_next_line_directive_edit_preserves_indentation() {
        let edit = only_edit(
            build_insert_disable_next_line_directive_edit(
                &test_uri(),
                &rope("    SELECT DISTINCT id FROM users;\n"),
                0,
                "no-distinct",
            )
            .unwrap(),
        );

        assert_eq!(
            edit.range,
            Range::new(Position::new(0, 0), Position::new(0, 0))
        );
        assert_eq!(
            edit.new_text,
            "    -- uroborosql-lint-disable-next-line no-distinct\n"
        );
    }

    #[test]
    fn build_insert_disable_next_line_directive_edit_preserves_crlf() {
        let edit = only_edit(
            build_insert_disable_next_line_directive_edit(
                &test_uri(),
                &rope("SELECT DISTINCT id FROM users;\r\n"),
                0,
                "no-distinct",
            )
            .unwrap(),
        );

        assert_eq!(
            edit.new_text,
            "-- uroborosql-lint-disable-next-line no-distinct\r\n"
        );
    }

    #[test]
    fn build_insert_disable_next_line_directive_edit_uses_target_line_ending_in_mixed_file() {
        let edit = only_edit(
            build_insert_disable_next_line_directive_edit(
                &test_uri(),
                &rope("SELECT 1;\r\nSELECT DISTINCT id FROM users;\n"),
                1,
                "no-distinct",
            )
            .unwrap(),
        );

        assert_eq!(
            edit.new_text,
            "-- uroborosql-lint-disable-next-line no-distinct\n"
        );
    }

    #[test]
    fn find_existing_disable_next_line_directive_appends_at_line_end() {
        let rope = rope("-- uroborosql-lint-disable-next-line no-distinct\nSELECT * FROM users;\n");
        let result =
            find_existing_disable_next_line_directive(&rope, 0, "no-wildcard-projection").unwrap();

        assert!(matches!(
            result,
            ExistingDisableNextLineDirective::Append(Position {
                line: 0,
                character: 48,
            })
        ));
    }

    #[test]
    fn find_existing_disable_next_line_directive_blocks_duplicate_rule() {
        let rope = rope(
            "-- uroborosql-lint-disable-next-line no-distinct\nSELECT DISTINCT id FROM users;\n",
        );
        let result = find_existing_disable_next_line_directive(&rope, 0, "no-distinct").unwrap();

        assert!(matches!(
            result,
            ExistingDisableNextLineDirective::AlreadyContains
        ));
    }

    #[test]
    fn find_existing_disable_next_line_directive_blocks_syntax_error() {
        let rope = rope(
            "-- uroborosql-lint-disable-next-line no-distinct,\nSELECT DISTINCT id FROM users;\n",
        );
        let result =
            find_existing_disable_next_line_directive(&rope, 0, "no-wildcard-projection").unwrap();

        assert!(matches!(result, ExistingDisableNextLineDirective::Blocked));
    }
}
