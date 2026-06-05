//! Cache request classification helpers.

use std::path::Path;

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, ProviderRegistrySnapshot, ResolvedProvider,
    syntax_query_ast_abi_fingerprint,
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
            &request.forwarded_args,
        ))),
        ClientMethod::Query => Some(CacheExportMethod::from(query_export_method(
            &request.forwarded_args,
        ))),
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

fn query_export_method(args: &[String]) -> String {
    if args
        .windows(2)
        .any(|window| window[0] == "--from-hook" && window[1] == "direct-source-read")
    {
        "query/direct-source-read".to_string()
    } else if has_tree_sitter_query(args) {
        "query/tree-sitter".to_string()
    } else {
        "query/owner-items".to_string()
    }
}

pub(super) fn has_tree_sitter_query(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--treesitter-query" || arg.starts_with("--treesitter-query="))
}

pub(super) fn request_lookup_fingerprint(
    provider: &ResolvedProvider,
    project_root: &Path,
    export_method: &CacheExportMethod,
    request: &ClientRequest,
) -> Option<String> {
    if export_method.as_str() == "query/tree-sitter" {
        Some(exact_request_fingerprint(
            provider,
            project_root,
            export_method,
            &request.forwarded_args,
        ))
    } else {
        None
    }
}

pub(super) fn exact_request_fingerprint(
    provider: &ResolvedProvider,
    project_root: &Path,
    export_method: &CacheExportMethod,
    forwarded_args: &[String],
) -> String {
    let syntax_query_provenance = syntax_query_cache_provenance(forwarded_args)
        .unwrap_or_else(|| "syntax-query-ast-abi:none".to_string());
    let seed = format!(
        "{}\0{}\0{}\0{}\0{}\0{}",
        provider.language_id,
        provider.provider_id,
        normalized_path(project_root),
        export_method,
        forwarded_args.join("\0"),
        syntax_query_provenance
    );
    format!("fnv64:{}", stable_hash_hex(&seed))
}

fn syntax_query_cache_provenance(forwarded_args: &[String]) -> Option<String> {
    let source = tree_sitter_query_source(forwarded_args)?;
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
