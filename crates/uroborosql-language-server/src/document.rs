use crate::Backend;
use ropey::Rope;
use tower_lsp_server::lsp_types::TextDocumentContentChangeEvent;
use tower_lsp_server::lsp_types::Uri;
use tower_lsp_server::lsp_types::{Position, Range};

#[derive(Clone)]
pub(crate) struct DocumentState {
    rope: Rope,
    version: i32,
}

pub(crate) fn rope_position_to_char_index(rope: &Rope, position: Position) -> Option<usize> {
    let line = position.line as usize;
    let utf16_col = position.character as usize;
    let line_count = rope.len_lines();

    if line > line_count {
        return None;
    }

    if line == line_count {
        return (utf16_col == 0).then_some(rope.len_chars());
    }

    let line_start = rope.line_to_char(line);
    let line_slice = rope.line(line);
    let char_idx = line_slice.try_utf16_cu_to_char(utf16_col).ok()?;
    Some(line_start + char_idx)
}

pub(crate) fn rope_char_index_to_position(rope: &Rope, idx: usize) -> Position {
    let clamped = idx.min(rope.len_chars());
    let line = rope.char_to_line(clamped);
    let line_start = rope.line_to_char(line);
    let char_offset = clamped - line_start;
    let utf16_col = rope.line(line).char_to_utf16_cu(char_offset);
    Position::new(line as u32, utf16_col as u32)
}

pub(crate) fn rope_range_to_char_index_range(rope: &Rope, range: &Range) -> Option<(usize, usize)> {
    let start = rope_position_to_char_index(rope, range.start)?;
    let end = rope_position_to_char_index(rope, range.end)?;
    Some((start, end))
}

pub(crate) fn rope_line_exists(rope: &Rope, line: u32) -> bool {
    (line as usize) < rope.len_lines()
}

pub(crate) fn rope_line_text_without_ending(rope: &Rope, line: u32) -> Option<String> {
    if !rope_line_exists(rope, line) {
        return None;
    }
    let mut text = rope.line(line as usize).to_string();
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    Some(text)
}

pub(crate) fn rope_line_has_ending(rope: &Rope, line: u32) -> bool {
    rope_line_exists(rope, line) && rope.line(line as usize).to_string().ends_with('\n')
}

pub(crate) fn rope_line_byte_to_position(
    rope: &Rope,
    line: u32,
    line_text: &str,
    byte: usize,
) -> Option<Position> {
    if !line_text.is_char_boundary(byte) {
        return None;
    }
    let line_start = rope.line_to_char(line as usize);
    let char_offset = line_text[..byte].chars().count();
    Some(rope_char_index_to_position(rope, line_start + char_offset))
}

pub(crate) fn rope_line_byte_range_to_range(
    rope: &Rope,
    line: u32,
    line_text: &str,
    range: std::ops::Range<usize>,
) -> Option<Range> {
    Some(Range::new(
        rope_line_byte_to_position(rope, line, line_text, range.start)?,
        rope_line_byte_to_position(rope, line, line_text, range.end)?,
    ))
}

impl Backend {
    pub(crate) fn upsert_document(&self, uri: &Uri, text: &str, version: Option<i32>) {
        let version = version
            .or_else(|| {
                self.documents
                    .read()
                    .ok()
                    .and_then(|docs| docs.get(uri).map(|doc| doc.version))
            })
            .unwrap_or_default();

        if let Ok(mut docs) = self.documents.write() {
            docs.insert(
                uri.clone(),
                DocumentState {
                    rope: Rope::from_str(text),
                    version,
                },
            );
        }
    }

    pub(crate) fn apply_change(
        &self,
        uri: &Uri,
        change: TextDocumentContentChangeEvent,
        version: i32,
    ) {
        if let Ok(mut docs) = self.documents.write()
            && let Some(doc) = docs.get_mut(uri)
        {
            if version < doc.version {
                return;
            }
            if let Some(range) = change.range {
                if let Some((start, end)) = rope_range_to_char_index_range(&doc.rope, &range) {
                    doc.rope.remove(start..end);
                    doc.rope.insert(start, &change.text);
                    doc.version = version;
                }
            } else {
                doc.rope = Rope::from_str(&change.text);
                doc.version = version;
            }
        }
    }

    pub(crate) fn remove_document(&self, uri: &Uri) {
        if let Ok(mut docs) = self.documents.write() {
            docs.remove(uri);
        }
    }

    pub(crate) fn document_rope(&self, uri: &Uri) -> Option<Rope> {
        self.documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).map(|doc| doc.rope.clone()))
    }

    pub(crate) fn document_text(&self, uri: &Uri) -> Option<String> {
        self.document_rope(uri).map(|rope| rope.to_string())
    }

    pub(crate) fn open_documents(&self) -> Vec<(Uri, String, i32)> {
        self.documents
            .read()
            .ok()
            .map(|docs| {
                docs.iter()
                    .map(|(uri, doc)| (uri.clone(), doc.rope.to_string(), doc.version))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn rope_line_text_without_ending_strips_lf_and_crlf() {
        let rope = rope("first\r\nsecond\nthird");

        assert_eq!(
            rope_line_text_without_ending(&rope, 0).as_deref(),
            Some("first")
        );
        assert_eq!(
            rope_line_text_without_ending(&rope, 1).as_deref(),
            Some("second")
        );
        assert_eq!(
            rope_line_text_without_ending(&rope, 2).as_deref(),
            Some("third")
        );
    }

    #[test]
    fn rope_line_byte_to_position_returns_utf16_position() {
        let rope = rope("😀 SELECT\n");
        let line_text = rope_line_text_without_ending(&rope, 0).unwrap();
        let byte_after_emoji = "😀".len();

        assert_eq!(
            rope_line_byte_to_position(&rope, 0, &line_text, byte_after_emoji),
            Some(Position::new(0, 2))
        );
    }
}
