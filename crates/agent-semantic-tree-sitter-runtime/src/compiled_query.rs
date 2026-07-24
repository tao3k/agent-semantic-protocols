use tree_sitter::{Language, Query};

const SUPPORTED_PREDICATES: &[&str] = &["eq?", "not-eq?", "match?", "not-match?", "any-of?"];

/// A compiled Tree-sitter query with stable capture metadata.
pub struct CompiledSyntaxQuery {
    query: Query,
    capture_names: Vec<String>,
    pattern_count: usize,
    unsupported_predicates: Vec<String>,
}

impl CompiledSyntaxQuery {
    /// Borrow the underlying Tree-sitter query for provider-specific use.
    #[must_use]
    pub fn query(&self) -> &Query {
        &self.query
    }

    /// Capture names declared by the query source.
    #[must_use]
    pub fn capture_names(&self) -> &[String] {
        &self.capture_names
    }

    /// Number of compiled patterns.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.pattern_count
    }

    /// Predicates that the canonical runtime cannot evaluate.
    #[must_use]
    pub fn unsupported_predicates(&self) -> &[String] {
        &self.unsupported_predicates
    }
}

impl std::fmt::Debug for CompiledSyntaxQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledSyntaxQuery")
            .field("capture_names", &self.capture_names)
            .field("pattern_count", &self.pattern_count)
            .field("unsupported_predicates", &self.unsupported_predicates)
            .finish_non_exhaustive()
    }
}

/// Compile a query for a provider-supplied language grammar.
pub fn compile_query_source(
    language: &Language,
    source: &str,
) -> Result<CompiledSyntaxQuery, String> {
    let query = Query::new(language, source)
        .map_err(|error| format!("failed to compile tree-sitter query: {error:?}"))?;
    let capture_names = query
        .capture_names()
        .iter()
        .map(ToString::to_string)
        .collect();
    let unsupported_predicates = unsupported_predicates(&query);
    Ok(CompiledSyntaxQuery {
        pattern_count: query.pattern_count(),
        query,
        capture_names,
        unsupported_predicates,
    })
}

fn unsupported_predicates(query: &Query) -> Vec<String> {
    let mut unsupported = Vec::new();
    for pattern_index in 0..query.pattern_count() {
        for predicate in query.general_predicates(pattern_index) {
            let operator = predicate.operator.trim_start_matches('#');
            if !SUPPORTED_PREDICATES.contains(&operator) {
                unsupported.push(operator.to_string());
            }
        }
    }
    unsupported.sort();
    unsupported.dedup();
    unsupported
}
