//! Language-neutral execution for ASP Tree-sitter queries.

use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

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

/// One capture returned by a canonical Tree-sitter query execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeQueryNode {
    pub node_kind: String,
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

/// One capture returned by a canonical Tree-sitter query execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeQueryCapture {
    pub capture_index: u32,
    pub capture_name: String,
    pub node: NativeQueryNode,
    /// The direct parent is first; providers choose their own enclosing item.
    pub ancestors: Vec<NativeQueryNode>,
}

/// One structural query match with its normalized captures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeQueryMatch {
    pub pattern_index: usize,
    pub captures: Vec<NativeQueryCapture>,
}

/// Query execution evidence that providers can project into their own packets.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeQueryExecution {
    pub matches: Vec<NativeQueryMatch>,
    pub parsed: bool,
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

/// Execute a compiled query with Tree-sitter's parser and `QueryCursor`.
pub fn execute_query(
    language: &Language,
    query: &CompiledSyntaxQuery,
    source: &str,
) -> Result<NativeQueryExecution, String> {
    let mut parser = Parser::new();
    parser
        .set_language(language)
        .map_err(|error| format!("failed to set tree-sitter language: {error}"))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "tree-sitter did not produce a syntax tree".to_string())?;
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query.query(), tree.root_node(), source.as_bytes());
    let mut output = Vec::new();
    while let Some(query_match) = matches.next() {
        let captures = query_match
            .captures
            .iter()
            .map(|capture| {
                let node = capture.node;
                let mut ancestors = Vec::new();
                let mut parent = node.parent();
                while let Some(ancestor) = parent {
                    ancestors.push(snapshot_node(ancestor, source)?);
                    parent = ancestor.parent();
                }
                Ok(NativeQueryCapture {
                    capture_index: capture.index,
                    capture_name: query
                        .capture_names()
                        .get(capture.index as usize)
                        .cloned()
                        .unwrap_or_else(|| format!("capture-{}", capture.index)),
                    node: snapshot_node(node, source)?,
                    ancestors,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        output.push(NativeQueryMatch {
            pattern_index: query_match.pattern_index,
            captures,
        });
    }
    Ok(NativeQueryExecution {
        matches: output,
        parsed: true,
    })
}

fn snapshot_node(node: tree_sitter::Node<'_>, source: &str) -> Result<NativeQueryNode, String> {
    let text = node
        .utf8_text(source.as_bytes())
        .map_err(|error| format!("invalid UTF-8 node text: {error}"))?
        .to_string();
    Ok(NativeQueryNode {
        node_kind: node.kind().to_string(),
        text,
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
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
