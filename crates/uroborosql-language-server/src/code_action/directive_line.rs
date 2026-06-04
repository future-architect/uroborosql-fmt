pub(in crate::code_action) fn leading_whitespace(text: &str) -> &str {
    let end = text
        .char_indices()
        .find_map(|(idx, ch)| (!matches!(ch, ' ' | '\t')).then_some(idx))
        .unwrap_or(text.len());
    &text[..end]
}

pub(in crate::code_action) fn directive_text_with_offset(line_text: &str) -> (&str, usize) {
    let offset = leading_whitespace(line_text).len();
    (&line_text[offset..], offset)
}

pub(in crate::code_action) fn add_offset(
    range: std::ops::Range<usize>,
    offset: usize,
) -> std::ops::Range<usize> {
    range.start + offset..range.end + offset
}
