use crate::diagnostic::Diagnostic;

/// Mutable linting context shared across rules.
pub struct LintContext {
    diagnostics: Vec<Diagnostic>,
}

impl LintContext {
    pub fn new(_source: &str) -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
