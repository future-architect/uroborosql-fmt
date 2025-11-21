use ropey::Rope;
use tower_lsp_server::lsp_types::{Position, Range};

/// Converts an LSP position (line/character) into a char index within the Rope.
/// Returns `None` if the requested line/character falls outside the current document.
pub fn rope_position_to_char(rope: &Rope, position: Position) -> Option<usize> {
    let line = position.line as usize;
    let column = position.character as usize;
    let line_count = rope.len_lines();

    if line > line_count {
        return None;
    }

    if line == line_count {
        return if column == 0 {
            Some(rope.len_chars())
        } else {
            None
        };
    }

    let line_start = rope.line_to_char(line);
    let line_len = rope.line(line).len_chars();
    if column > line_len {
        None
    } else {
        Some(line_start + column)
    }
}

/// Converts a Rope char index into an LSP position (line/character).
/// Clamps the index to the end of the document if it exceeds `len_chars`.
pub fn rope_char_to_position(rope: &Rope, idx: usize) -> Position {
    let total_chars = rope.len_chars();
    let clamped = idx.min(total_chars);
    let line = rope.char_to_line(clamped);
    let line_start = rope.line_to_char(line);
    Position::new(line as u32, (clamped - line_start) as u32)
}

/// Converts an LSP range into a pair of Rope char indices.
/// Returns `None` if either endpoint of the range is invalid within the document.
pub fn rope_range_to_char_range(rope: &Rope, range: &Range) -> Option<(usize, usize)> {
    let start = rope_position_to_char(rope, range.start)?;
    let end = rope_position_to_char(rope, range.end)?;
    Some((start, end))
}
