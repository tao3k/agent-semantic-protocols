//! ASP-side preflight validation for query requests.

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, builtin_catalog_source, compile_query_abi_source,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn validate_syntax_query_request(request: &ClientRequest) -> Result<(), String> {
    validate_code_flag_boundary(request)?;
    if request.method != ClientMethod::Query {
        return Ok(());
    }
    validate_query_owner_path(request)?;
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

fn validate_query_owner_path(request: &ClientRequest) -> Result<(), String> {
    let Some(owner) = query_owner_path_arg(&request.forwarded_args) else {
        return Ok(());
    };
    let workspace = query_workspace(request);
    let owner_path = resolve_under_workspace(&workspace, owner);
    if !owner_path.exists() {
        return Err(format!(
            "query owner path does not exist under --workspace: {owner} workspace={}",
            workspace.display()
        ));
    }
    let workspace = canonical_or_original(workspace);
    let owner_path = canonical_or_original(owner_path);
    if !owner_path.starts_with(&workspace) {
        return Err(format!(
            "query owner path is outside --workspace: {owner} workspace={}",
            workspace.display()
        ));
    }
    Ok(())
}

fn query_owner_path_arg(args: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        match arg {
            "--catalog" | "--from-hook" | "--query" | "--selector" | "--term"
            | "--treesitter-query" | "--workspace" => {
                index += 2;
                continue;
            }
            "--json" => {
                index += optional_value_flag_width(args.get(index + 1).map(String::as_str));
                continue;
            }
            "--code" | "--names-only" => {
                index += 1;
                continue;
            }
            _ if arg.starts_with("--catalog=")
                || arg.starts_with("--from-hook=")
                || arg.starts_with("--query=")
                || arg.starts_with("--json=")
                || arg.starts_with("--selector=")
                || arg.starts_with("--term=")
                || arg.starts_with("--treesitter-query=")
                || arg.starts_with("--workspace=") =>
            {
                index += 1;
                continue;
            }
            _ if arg.starts_with('-') => {
                index += 1;
                continue;
            }
            _ if looks_like_owner_path(arg) => return Some(arg),
            _ => {
                index += 1;
            }
        }
    }
    None
}

fn optional_value_flag_width(next: Option<&str>) -> usize {
    if next.is_some_and(|arg| !arg.starts_with('-')) {
        2
    } else {
        1
    }
}

fn looks_like_owner_path(value: &str) -> bool {
    value != "." && (value.contains('/') || value.contains('\\'))
}

fn query_workspace(request: &ClientRequest) -> PathBuf {
    option_value(&request.forwarded_args, "--workspace")
        .map(|workspace| resolve_under_workspace(&request.project_root, workspace))
        .unwrap_or_else(|| request.project_root.clone())
}

fn option_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let eq_prefix = format!("{name}=");
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == name {
            return args.get(index + 1).map(String::as_str);
        }
        if let Some(value) = arg.strip_prefix(&eq_prefix) {
            return Some(value);
        }
        index += 1;
    }
    None
}

fn resolve_under_workspace(workspace: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

fn canonical_or_original(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
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
