use crate::document::{rope_byte_to_position, rope_char_index_to_position};
use tower_lsp_server::lsp_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
};
use uroborosql_lint::{
    Diagnostic as SqlDiagnostic, LINT_SOURCE, LintError, Severity as SqlSeverity,
};

pub(crate) fn to_lsp_diagnostic(diag: SqlDiagnostic, rope: Option<&ropey::Rope>) -> Diagnostic {
    let severity = match diag.severity {
        SqlSeverity::Error => Some(DiagnosticSeverity::ERROR),
        SqlSeverity::Warning => Some(DiagnosticSeverity::WARNING),
        SqlSeverity::Info => Some(DiagnosticSeverity::INFORMATION),
    };

    let range = if let Some(rope) = rope {
        Range {
            start: rope_byte_to_position(rope, diag.span.start.byte),
            end: rope_byte_to_position(rope, diag.span.end.byte),
        }
    } else {
        Range {
            start: Position::new(diag.span.start.line as u32, diag.span.start.column as u32),
            end: Position::new(diag.span.end.line as u32, diag.span.end.column as u32),
        }
    };

    Diagnostic {
        range,
        severity,
        code: Some(NumberOrString::String(diag.code.to_string())),
        source: Some(LINT_SOURCE.into()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

pub(crate) fn to_parse_error(err: LintError, rope: Option<&ropey::Rope>) -> Diagnostic {
    let LintError::ParseError { message, span } = err;

    let range = match (rope, span) {
        (Some(rope), Some(span)) => {
            let start = rope_byte_to_position(rope, span.start_byte);
            let end = rope_byte_to_position(rope, span.end_byte);
            Range { start, end }
        }
        // Unknown position: point at the end of the file (zero-width range).
        (Some(rope), None) => {
            let eof = rope_char_index_to_position(rope, rope.len_chars());
            Range {
                start: eof,
                end: eof,
            }
        }
        (None, _) => Range::default(),
    };

    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        source: Some(LINT_SOURCE.into()),
        message: format!("Failed to parse SQL: {message}"),
        ..Diagnostic::default()
    }
}
