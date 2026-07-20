//! Cache request classification helpers.

use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, ProviderRegistrySnapshot, ResolvedProvider,
    builtin_catalog_source, syntax_query_ast_abi_fingerprint,
};

pub(super) fn selected_provider_for_request<'a>(
    snapshot: &'a ProviderRegistrySnapshot,
    request: &ClientRequest,
) -> Option<&'a ResolvedProvider> {
    if let Some(language_id) = &request.language_id {
        return snapshot.provider_for_language(language_id);
    }
    if snapshot.providers.len() == 1 {
        snapshot.providers.first()
    } else {
        None
    }
}

pub(super) fn request_export_method(request: &ClientRequest) -> Option<CacheExportMethod> {
    match &request.method {
        ClientMethod::Search => Some(CacheExportMethod::from(search_export_method(
            search_cache_forwarded_args(&request.forwarded_args).as_ref(),
        ))),
        ClientMethod::Query => Some(CacheExportMethod::from(query_export_method(request))),
        ClientMethod::Check => Some(CacheExportMethod::from(check_export_method(
            &request.forwarded_args,
        ))),
        _ => None,
    }
}

fn search_export_method(args: &[String]) -> String {
    args.first()
        .filter(|arg| !arg.starts_with('-') && arg.as_str() != ".")
        .map_or_else(|| "search".to_string(), |arg| format!("search/{arg}"))
}

pub(crate) fn search_cache_forwarded_args(args: &[String]) -> std::borrow::Cow<'_, [String]> {
    let dependency_alias = args.first().is_some_and(|arg| arg == "dependency");
    let trailing_workspace_marker = args.last().is_some_and(|arg| {
        if arg == "." {
            return true;
        }
        let path = std::path::Path::new(arg);
        path.is_absolute() && path.canonicalize().is_ok_and(|path| path.is_dir())
    });
    if dependency_alias || trailing_workspace_marker {
        let mut normalized = args.to_vec();
        if dependency_alias {
            normalized[0] = "deps".to_string();
        }
        if trailing_workspace_marker {
            normalized.pop();
        }
        std::borrow::Cow::Owned(normalized)
    } else {
        std::borrow::Cow::Borrowed(args)
    }
}

fn query_export_method(request: &ClientRequest) -> String {
    let args = &request.forwarded_args;
    if has_tree_sitter_query(args) {
        "query/tree-sitter".to_string()
    } else if is_selector_code_query(args) {
        "query/code".to_string()
    } else {
        "query/owner-items".to_string()
    }
}

fn is_selector_code_query(args: &[String]) -> bool {
    args.windows(2).any(|window| window[0] == "--selector")
        && args.iter().any(|arg| arg == "--code")
}

pub(super) fn has_tree_sitter_query(args: &[String]) -> bool {
    args.iter().any(|arg| {
        arg == "--treesitter-query"
            || arg.starts_with("--treesitter-query=")
            || arg == "--catalog"
            || arg.starts_with("--catalog=")
    })
}

pub(super) fn request_lookup_fingerprint(
    provider: &ResolvedProvider,
    project_root: &Path,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<String> {
    if export_method.as_str() == "query/tree-sitter" {
        let _ = (provider, project_root, request);
        None
    } else {
        Some(exact_request_fingerprint(
            provider,
            project_root,
            export_method,
            &request.forwarded_args,
        ))
    }
}

pub(super) fn exact_request_fingerprint(
    provider: &ResolvedProvider,
    project_root: &Path,
    export_method: &CacheExportMethod,
    forwarded_args: &[String],
) -> String {
    let forwarded_args = search_cache_forwarded_args(forwarded_args);
    let syntax_query_provenance = syntax_query_cache_provenance(provider, forwarded_args.as_ref())
        .unwrap_or_else(|| "syntax-query-ast-abi:none".to_string());
    let prompt_output_provenance = prompt_output_render_abi_provenance(export_method);
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        forwarded_args.join("\0"),
        syntax_query_provenance,
        prompt_output_provenance
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

pub(super) fn prompt_output_render_abi_provenance(export_method: &CacheExportMethod) -> String {
    if matches!(export_method.as_str(), "search/prime" | "search/package") {
        return format!(
            "prompt-output-render-abi:fnv64:{}",
            stable_hash_hex(PRIME_DECISION_PRIMER_RENDER_ABI)
        );
    }
    "prompt-output-render-abi:none".to_string()
}

const PRIME_DECISION_PRIMER_RENDER_ABI: &str = concat!(
    "semantic-search-prime;",
    "purpose=decision-primer;",
    "answer=false;",
    "code=false;",
    "capabilities=pipe,lexical,owner-items,selector-code,treesitter-query;",
    "ladder=pipe>lexical>owner-items>selector-code;",
    "history=asp-artifacts:directReadRisk,repeatedPrime,repeatedPipe,bestPath;",
    "risk=broad-direct-read,manual-window-scan,repeat-prime;",
    "next=search pipe <question-or-feature-term> --view seeds"
);

pub(super) fn syntax_query_cache_provenance(
    provider: &ResolvedProvider,
    forwarded_args: &[String],
) -> Option<String> {
    let source = tree_sitter_query_source(forwarded_args).or_else(|| {
        let catalog_id = tree_sitter_catalog_id(forwarded_args)?;
        builtin_catalog_source(provider.language_id.as_str().into(), catalog_id.into())
    })?;
    syntax_query_ast_abi_fingerprint(source).ok()
}

fn check_export_method(args: &[String]) -> String {
    if args
        .iter()
        .any(|arg| arg == "--changed" || arg == "changed")
    {
        "check/changed".to_string()
    } else if args.iter().any(|arg| arg == "--full" || arg == "full") {
        "check/full".to_string()
    } else {
        "check".to_string()
    }
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

fn normalized_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn stable_hash_hex(value: &str) -> String {
    stable_hash_bytes(value.as_bytes())
}

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/request.rs"]
mod request_tests;

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/request_alias.rs"]
mod request_alias_tests;
