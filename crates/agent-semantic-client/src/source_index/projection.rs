//! Shared lifecycle import for query-free language-harness projections.



/// Result of validating and importing one parser-owned language projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct LanguageProjectionImportReport {
    reused: bool,
    node_locator_count: usize,
}

impl LanguageProjectionImportReport {
    /// Whether the existing source-index generation was reused.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }

    /// Number of node locators imported for a new generation.
    #[must_use]
    pub const fn node_locator_count(&self) -> usize {
        self.node_locator_count
    }
}
