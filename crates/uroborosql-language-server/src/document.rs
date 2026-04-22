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
