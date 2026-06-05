//! ASP-side preflight validation for tree-sitter query ABI requests.

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, builtin_catalog_source, compile_query_abi_source,
};

pub(crate) fn validate_syntax_query_request(request: &ClientRequest) -> Result<(), String> {
    if request.method != ClientMethod::Query {
        return Ok(());
    }
    let Some(source) = tree_sitter_query_source(request)? else {
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

fn tree_sitter_query_source(request: &ClientRequest) -> Result<Option<&str>, String> {
    let mut iter = request.forwarded_args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--treesitter-query" {
            return Ok(iter.next().map(String::as_str));
        }
        if let Some(value) = arg.strip_prefix("--treesitter-query=") {
            return Ok(Some(value));
        }
    }
    let Some(catalog_id) = tree_sitter_catalog_id(&request.forwarded_args) else {
        return Ok(None);
    };
    let Some(language_id) = request.language_id.as_ref() else {
        return Ok(None);
    };
    builtin_catalog_source(language_id.as_str().into(), catalog_id.into())
        .map(Some)
        .ok_or_else(|| {
            format!(
                "unknown built-in tree-sitter query catalog `{catalog_id}` for language `{}`",
                language_id.as_str()
            )
        })
}

fn tree_sitter_catalog_id(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--catalog" {
            return iter.next().map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix("--catalog=") {
            return Some(value);
        }
    }
    None
}
