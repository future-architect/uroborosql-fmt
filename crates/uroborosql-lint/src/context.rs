use crate::{diagnostic::Diagnostic, resolution::Catalog};

/// Mutable linting context shared across rules.
pub struct LintContext<'a> {
    diagnostics: Vec<Diagnostic>,
    catalog: Option<Catalog<'a>>,
}

impl<'a> LintContext<'a> {
    pub fn new(_source: &str) -> Self {
        Self {
            diagnostics: Vec::new(),
            catalog: None,
        }
    }

    pub(crate) fn with_catalog(source: &str, catalog: Catalog<'a>) -> Self {
        Self {
            catalog: Some(catalog),
            ..Self::new(source)
        }
    }

    /// Acquired catalog state; `None` when rules run without a catalog.
    pub(crate) fn catalog(&self) -> Option<&Catalog<'a>> {
        self.catalog.as_ref()
    }

    pub fn report(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
