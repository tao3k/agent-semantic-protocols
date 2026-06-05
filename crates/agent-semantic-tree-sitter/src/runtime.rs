//! Runtime compilation for tree-sitter-compatible query catalog sources.
//!
//! ASP owns this shared runtime boundary. Grammar-specific crates are supplied by
//! callers that need runtime matching; language providers can still project
//! native parser facts without linking tree-sitter.

use crate::LoadedSyntaxCatalog;

/// A compiled tree-sitter query plus stable metadata needed by cache/receipt layers.
pub struct CompiledSyntaxQuery {
    query: tree_sitter::Query,
    capture_names: Vec<String>,
    pattern_count: usize,
}

impl CompiledSyntaxQuery {
    /// Borrow the underlying tree-sitter query for runtime matching.
    #[must_use]
    pub fn query(&self) -> &tree_sitter::Query {
        &self.query
    }

    /// Capture names reported by the tree-sitter query compiler.
    #[must_use]
    pub fn capture_names(&self) -> &[String] {
        &self.capture_names
    }

    /// Number of compiled patterns in the query source.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.pattern_count
    }

    fn from_query(query: tree_sitter::Query) -> Self {
        let capture_names = query
            .capture_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let pattern_count = query.pattern_count();
        Self {
            query,
            capture_names,
            pattern_count,
        }
    }
}

impl std::fmt::Debug for CompiledSyntaxQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledSyntaxQuery")
            .field("capture_names", &self.capture_names)
            .field("pattern_count", &self.pattern_count)
            .finish_non_exhaustive()
    }
}

/// Error returned when tree-sitter rejects a catalog query source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxQueryCompileError {
    /// Compiler-owned diagnostic text.
    pub message: String,
}

impl std::fmt::Display for SyntaxQueryCompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SyntaxQueryCompileError {}

/// Compile an arbitrary tree-sitter-compatible query source with a caller-supplied language.
pub fn compile_query_source(
    language: &tree_sitter::Language,
    source: &str,
) -> Result<CompiledSyntaxQuery, SyntaxQueryCompileError> {
    let query =
        tree_sitter::Query::new(language, source).map_err(|error| SyntaxQueryCompileError {
            message: format!("{error:?}"),
        })?;
    Ok(CompiledSyntaxQuery::from_query(query))
}

/// Compile a loaded catalog source with a caller-supplied language.
pub fn compile_catalog_query(
    language: &tree_sitter::Language,
    catalog: &LoadedSyntaxCatalog,
) -> Result<CompiledSyntaxQuery, SyntaxQueryCompileError> {
    compile_query_source(language, &catalog.source)
}
