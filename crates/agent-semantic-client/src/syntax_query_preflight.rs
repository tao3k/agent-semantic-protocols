//! ASP-side preflight validation for query requests.

use agent_semantic_client_core::{
    ClientMethod, ClientRequest, builtin_catalog_source, compile_query_abi_source,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Validate client-side syntax query boundaries before provider execution.
pub fn validate_syntax_query_request(request: &ClientRequest) -> Result<(), String> {
    if request.method != ClientMethod::Query {
        return Ok(());
    }
    validate_exact_projection_selector_target(request)?;
    validate_query_owner_path(request)?;
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

fn validate_exact_projection_selector_target(request: &ClientRequest) -> Result<(), String> {
    if option_value(&request.forwarded_args, "--projection").is_none() {
        return Ok(());
    }
    let Some(selector) = option_value(&request.forwarded_args, "--selector") else {
        return Ok(());
    };
    let workspace = query_workspace(request);
    let selector_owner = selector_owner_path(selector);
    let selector_path =
        resolve_under_workspace(&workspace, selector_owner.as_deref().unwrap_or(selector));
    if selector_path.is_dir() {
        return Err(format!(
            "exact query requires a parser-owned structural selector; `{selector}` is a directory. Use ASP Search with --workspace for directory-scoped discovery"
        ));
    }
    if tree_sitter_query_source(request)?.is_some() {
        return Ok(());
    }
    if let Some(language_id) = non_structural_selector_language(request, selector) {
        let workspace_arg = query_workspace_arg(request).unwrap_or(".");
        return Err(format!(
            "invalid exact-query selector `{selector}`: file selectors are not executable structural selectors; query an exact parser-owned item selector such as {language_id}://path#item/function/name; recover through ASP Search\nselectorState=file-selector\nallowed=false\nreason=file-selectors-are-not-structural-selectors\nnextAction=run-search\nnextCommand=asp {language_id} search 'source structure' --scope owner:{selector} --workspace {workspace_arg}\nrequiredSelector={language_id}://{selector}#item/<kind>/<name>"
        ));
    }
    if let Some(owner) = selector_owner.as_deref() {
        if !selector_path.exists() {
            return Err(format!(
                "stale-index selector path does not exist under --workspace: {owner} selector={selector} workspace={}",
                workspace.display()
            ));
        }
        let workspace = canonical_or_original(workspace);
        let selector_path = canonical_or_original(selector_path);
        if !selector_path.starts_with(&workspace) {
            return Err(format!(
                "selector path is outside --workspace: {owner} selector={selector} workspace={}",
                workspace.display()
            ));
        }
    }
    Ok(())
}

fn selector_owner_path(selector: &str) -> Option<String> {
    agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(selector)
        .ok()?
        .owner_path()
        .ok()
}

fn selector_path_before_range(selector: &str) -> &str {
    selector
        .split_once(':')
        .map_or(selector, |(path, _range)| path)
}

fn non_structural_selector_language<'a>(
    request: &'a ClientRequest,
    selector: &str,
) -> Option<&'a str> {
    if selector_path_before_range(selector) != selector {
        return None;
    }
    if agent_semantic_content_identity::CanonicalItemSelector::parse_root_or_exact_descendant(
        selector,
    )
    .is_ok()
    {
        return None;
    }
    request
        .language_id
        .as_ref()
        .map(|language| language.as_str())
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
            "--catalog" | "--from-hook" | "--projection" | "--query" | "--selector" | "--term"
            | "--treesitter-query" | "--workspace" => {
                index += 2;
                continue;
            }
            "--json" => {
                index += optional_value_flag_width(args.get(index + 1).map(String::as_str));
                continue;
            }
            _ if arg.starts_with("--catalog=")
                || arg.starts_with("--from-hook=")
                || arg.starts_with("--projection=")
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
    query_workspace_arg(request)
        .map(|workspace| resolve_under_workspace(&request.project_root, workspace))
        .unwrap_or_else(|| request.project_root.clone())
}

fn query_workspace_arg(request: &ClientRequest) -> Option<&str> {
    option_value(&request.forwarded_args, "--workspace")
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

fn is_native_query_catalog(catalog_id: &str) -> bool {
    matches!(catalog_id, "flow-lite")
}
