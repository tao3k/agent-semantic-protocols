//! Request model passed from `agent-semantic-client` to execution backends.

use std::path::PathBuf;

use crate::types::LanguageId;
use agent_semantic_tree_sitter::compile_query_abi_source;
use serde::{Deserialize, Serialize};

/// Internal ASP-to-provider argument carrying query capture names.
///
/// This is not a public query surface. It lets the ASP client compile the
/// tree-sitter-compatible query ABI before native provider projection.
pub const ASP_SYNTAX_QUERY_CAPTURES_ARG: &str = "--asp-syntax-query-captures";

/// Internal ASP-to-provider argument carrying query node types.
pub const ASP_SYNTAX_QUERY_NODE_TYPES_ARG: &str = "--asp-syntax-query-node-types";

/// Internal ASP-to-provider argument carrying query field names.
pub const ASP_SYNTAX_QUERY_FIELDS_ARG: &str = "--asp-syntax-query-fields";

/// Shared agent-facing method routed by the client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientMethod {
    Guide,
    Providers,
    Doctor,
    CacheStatus,
    CacheImport,
    CacheInvalidate,
    Search,
    Query,
    Check,
}

/// Client request sent to a local, cache, or future cloud backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientRequest {
    pub method: ClientMethod,
    pub language_id: Option<LanguageId>,
    pub forwarded_args: Vec<String>,
    pub project_root: PathBuf,
}

impl ClientRequest {
    /// Create a request for a method and project root.
    #[must_use]
    pub fn new(method: ClientMethod, project_root: impl Into<PathBuf>) -> Self {
        Self {
            method,
            language_id: None,
            forwarded_args: Vec::new(),
            project_root: project_root.into(),
        }
    }

    /// Attach an explicit language id to the request.
    #[must_use]
    pub fn with_language(mut self, language_id: impl Into<LanguageId>) -> Self {
        self.language_id = Some(language_id.into());
        self
    }

    /// Attach provider-native forwarded arguments.
    #[must_use]
    pub fn with_forwarded_args(mut self, forwarded_args: Vec<String>) -> Self {
        self.forwarded_args = forwarded_args;
        self
    }
}

/// Append ASP-compiled tree-sitter query ABI metadata for native projection.
pub fn append_syntax_query_plan_args(
    method: &ClientMethod,
    forwarded_args: Vec<String>,
) -> Result<Vec<String>, String> {
    if *method != ClientMethod::Query
        || forwarded_args
            .iter()
            .any(|arg| arg == ASP_SYNTAX_QUERY_CAPTURES_ARG)
    {
        return Ok(forwarded_args);
    }
    let Some(source) = tree_sitter_query_source(&forwarded_args) else {
        return Ok(forwarded_args);
    };
    let plan = compile_query_abi_source(source).map_err(|error| {
        format!(
            "invalid tree-sitter query ABI source before provider execution: {}",
            error.message
        )
    })?;
    let mut enriched_args = forwarded_args;
    enriched_args.push(ASP_SYNTAX_QUERY_CAPTURES_ARG.to_string());
    enriched_args.push(plan.captures.join(","));
    enriched_args.push(ASP_SYNTAX_QUERY_NODE_TYPES_ARG.to_string());
    enriched_args.push(plan.node_types.join(","));
    enriched_args.push(ASP_SYNTAX_QUERY_FIELDS_ARG.to_string());
    enriched_args.push(plan.fields.join(","));
    Ok(enriched_args)
}

fn tree_sitter_query_source(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--treesitter-query" {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix("--treesitter-query=") {
            return Some(value);
        }
    }
    None
}
