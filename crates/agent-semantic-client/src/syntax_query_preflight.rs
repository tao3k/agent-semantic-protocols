//! ASP-side preflight validation for tree-sitter query ABI requests.

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, builtin_catalog_source, compile_query_abi_source,
};

pub(crate) fn validate_syntax_query_request(request: &ClientRequest) -> Result<(), String> {
    validate_code_flag_boundary(request)?;
    if request.method != ClientMethod::Query {
        return Ok(());
    }
    let Some(source) = tree_sitter_query_source(request)? else {
        return Ok(());
    };
    if requests_code_output(&request.forwarded_args) && !has_exact_selector(&request.forwarded_args)
    {
        return Err(
            "tree-sitter query --code requires an exact --selector; run without --code for a capture frontier or add --selector <path-or-range> for pure code"
                .to_string(),
        );
    }
    compile_query_abi_source(source).map_err(|error| {
        format!(
            "invalid tree-sitter query ABI source before provider execution: {}",
            error.message
        )
    })?;
    Ok(())
}

fn validate_code_flag_boundary(request: &ClientRequest) -> Result<(), String> {
    if !matches!(request.method, ClientMethod::Query | ClientMethod::Search) {
        return Ok(());
    }
    for window in request.forwarded_args.windows(2) {
        if window[0] == "--code" && !window[1].starts_with('-') {
            return Err(
                "query/search --code does not accept a trailing PROJECT_ROOT; use --workspace PROJECT_ROOT"
                    .to_string(),
            );
        }
    }
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
            return iter
                .next()
                .map(String::as_str)
                .filter(|catalog_id| !is_native_query_catalog(catalog_id));
        }
        if let Some(value) = arg.strip_prefix("--catalog=") {
            return (!is_native_query_catalog(value)).then_some(value);
        }
    }
    None
}

fn requests_code_output(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--code")
}

fn has_exact_selector(args: &[String]) -> bool {
    args.windows(2)
        .any(|window| window[0] == "--selector" && !window[1].starts_with('-'))
        || args
            .iter()
            .any(|arg| arg.starts_with("--selector=") && arg.len() > "--selector=".len())
}

fn is_native_query_catalog(catalog_id: &str) -> bool {
    matches!(catalog_id, "flow-lite")
}
