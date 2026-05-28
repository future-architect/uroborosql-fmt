mod config;
mod context;
mod diagnostic;
mod directive;
mod linter;
mod rule;
mod rules;
mod tree;

pub use config::{
    ConfigError, ConfigStore, ResolvedLintConfig, RuleLevel, RuleSetting, DEFAULT_CONFIG_FILENAME,
};
pub use diagnostic::{Diagnostic, Severity, SqlSpan};
pub use directive::{
    parse_line_comment_directive, DirectiveParseDiagnostic, DirectiveParseDiagnosticKind,
    ParsedLineComment, ParsedLintDirectiveKind, DISABLE_DIRECTIVE_KEYWORD,
    DISABLE_NEXT_LINE_DIRECTIVE_KEYWORD, INVALID_LINT_DIRECTIVE_CODE, LINT_SOURCE,
};
pub use linter::{LintError, Linter};
pub use rules::RuleEnum;
