//! Request model passed from `agent-semantic-client` to execution backends.

use std::path::PathBuf;

use crate::types::LanguageId;
use agent_semantic_tree_sitter::{
    SyntaxQueryAbiPredicate, SyntaxQueryPredicateValue, compile_query_abi_source,
};
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

/// Internal ASP-to-provider argument carrying structured query predicate ABI entries.
pub const ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG: &str = "--asp-syntax-query-predicates-json";

/// Versioned cache identity seed for tree-sitter-compatible query AST/ABI facts.
pub const SYNTAX_QUERY_AST_ABI_FINGERPRINT_VERSION: &str = "syntax-query-ast-abi.v1";

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
    if !plan.predicates.is_empty() {
        enriched_args.push(ASP_SYNTAX_QUERY_PREDICATES_JSON_ARG.to_string());
        enriched_args.push(syntax_query_predicates_json(&plan.predicates)?);
    }
    enriched_args.push(ASP_SYNTAX_QUERY_CAPTURES_ARG.to_string());
    enriched_args.push(plan.captures.join(","));
    enriched_args.push(ASP_SYNTAX_QUERY_NODE_TYPES_ARG.to_string());
    enriched_args.push(plan.node_types.join(","));
    enriched_args.push(ASP_SYNTAX_QUERY_FIELDS_ARG.to_string());
    enriched_args.push(plan.fields.join(","));
    Ok(enriched_args)
}

/// Return the stable query AST/ABI fingerprint used by client cache boundaries.
pub fn syntax_query_ast_abi_fingerprint(source: &str) -> Result<String, String> {
    let plan = compile_query_abi_source(source).map_err(|error| error.message)?;
    let predicates = plan
        .predicates
        .iter()
        .map(syntax_query_predicate_fingerprint_key)
        .collect::<Vec<_>>()
        .join(",");
    let seed = format!(
        "{}\0patterns={}\0captures={}\0nodeTypes={}\0fields={}\0predicates={}",
        SYNTAX_QUERY_AST_ABI_FINGERPRINT_VERSION,
        plan.pattern_count(),
        plan.captures.join(","),
        plan.node_types.join(","),
        plan.fields.join(","),
        predicates
    );
    Ok(format!(
        "syntax-query-ast-abi:{}",
        stable_hash_bytes(seed.as_bytes())
    ))
}

fn syntax_query_predicates_json(predicates: &[SyntaxQueryAbiPredicate]) -> Result<String, String> {
    let predicates = predicates
        .iter()
        .map(|predicate| {
            serde_json::json!({
                "op": predicate.op.as_abi_str(),
                "capture": predicate.capture.as_str(),
                "values": predicate.values.iter().map(|value| match value {
                    SyntaxQueryPredicateValue::String(value) => serde_json::json!({
                        "kind": "string",
                        "value": value.as_str()
                    }),
                    SyntaxQueryPredicateValue::Capture(value) => serde_json::json!({
                        "kind": "capture",
                        "value": value.as_str()
                    }),
                }).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&predicates)
        .map_err(|error| format!("failed to serialize ASP syntax query predicates: {error}"))
}

fn syntax_query_predicate_fingerprint_key(predicate: &SyntaxQueryAbiPredicate) -> String {
    let values = predicate
        .values
        .iter()
        .map(|value| match value {
            SyntaxQueryPredicateValue::String(value) => {
                format!("string:{}", escape_fingerprint_component(value))
            }
            SyntaxQueryPredicateValue::Capture(value) => {
                format!("capture:{}", escape_fingerprint_component(value))
            }
        })
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "{}:{}=[{}]",
        predicate.op.as_abi_str(),
        escape_fingerprint_component(&predicate.capture),
        values
    )
}

fn escape_fingerprint_component(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(',', "\\,")
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

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
