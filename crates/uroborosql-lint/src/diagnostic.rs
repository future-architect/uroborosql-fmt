use std::fmt;

use postgresql_cst_parser::tree_sitter::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
    pub byte: usize,
}

/// 1-based position for display. The column counts characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneBasedPosition {
    pub line: usize,
    pub column: usize,
}

impl OneBasedPosition {
    /// Locates the byte offset within `text`. The column counts characters.
    ///
    /// An out-of-range or non-boundary `byte` is rounded down to the nearest char boundary.
    pub fn from_byte_offset(text: &str, byte: usize) -> Self {
        let mut boundary = byte.min(text.len());
        while !text.is_char_boundary(boundary) {
            boundary -= 1;
        }

        let prefix = &text[..boundary];
        let line = prefix.matches('\n').count() + 1;
        let column = prefix.rsplit('\n').next().map_or(0, |s| s.chars().count()) + 1;
        Self { line, column }
    }
}

impl From<Position> for OneBasedPosition {
    fn from(position: Position) -> Self {
        Self {
            line: position.line + 1,
            column: position.column + 1,
        }
    }
}

impl fmt::Display for OneBasedPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqlSpan {
    pub start: Position,
    pub end: Position,
}

impl SqlSpan {
    pub fn from_range(range: &Range) -> Self {
        SqlSpan {
            start: Position {
                line: range.start_position.row,
                column: range.start_position.column,
                byte: range.start_byte,
            },
            end: Position {
                line: range.end_position.row,
                column: range.end_position.column,
                byte: range.end_byte,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub severity: Severity,
    pub span: SqlSpan,
}

impl Diagnostic {
    pub fn new(
        code: &'static str,
        severity: Severity,
        message: impl Into<String>,
        range: &Range,
    ) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            span: SqlSpan::from_range(range),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OneBasedPosition;

    fn pos(line: usize, column: usize) -> OneBasedPosition {
        OneBasedPosition { line, column }
    }

    #[test]
    fn from_byte_offset_is_one_based() {
        let text = "SELECT id\nFROM users";
        assert_eq!(OneBasedPosition::from_byte_offset(text, 0), pos(1, 1));
        assert_eq!(OneBasedPosition::from_byte_offset(text, 7), pos(1, 8));
        assert_eq!(OneBasedPosition::from_byte_offset(text, 10), pos(2, 1));
    }

    #[test]
    fn from_byte_offset_counts_columns_in_characters() {
        let text = "あいう x";
        // `x` is the 5th character: three multibyte chars plus a space.
        let byte = "あいう ".len();
        assert_eq!(OneBasedPosition::from_byte_offset(text, byte), pos(1, 5));
    }

    #[test]
    fn from_byte_offset_clamps_out_of_range_byte() {
        let text = "SELECT";
        assert_eq!(OneBasedPosition::from_byte_offset(text, 999), pos(1, 7));
    }
}
