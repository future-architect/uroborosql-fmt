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

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub rule_id: &'static str,
    pub message: String,
    pub severity: Severity,
    pub span: SqlSpan,
}

impl Diagnostic {
    pub fn new(
        rule_id: &'static str,
        severity: Severity,
        message: impl Into<String>,
        range: &Range,
    ) -> Self {
        Self {
            rule_id,
            severity,
            message: message.into(),
            span: SqlSpan::from_range(range),
        }
    }
}
