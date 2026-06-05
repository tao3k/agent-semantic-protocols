//! ASP-side preflight validation for tree-sitter query ABI requests.

use agent_semantic_client_core::{ClientMethod, ClientRequest, compile_query_abi_source};

pub(crate) fn validate_syntax_query_request(request: &ClientRequest) -> Result<(), String> {
    if request.method != ClientMethod::Query {
        return Ok(());
    }
    let Some(source) = tree_sitter_query_source(&request.forwarded_args) else {
        return Ok(());
    };
    compile_query_abi_source(source).map_err(|error| {
        format!(
            "invalid tree-sitter query ABI source before provider execution: {}",
            error.message
        )
    })?;
    Ok(())
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
