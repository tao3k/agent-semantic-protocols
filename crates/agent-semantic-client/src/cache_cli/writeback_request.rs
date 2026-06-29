//! Request classification for cache write-back.

use agent_semantic_client_core::{CacheExportMethod, ClientMethod, ClientRequest};

use super::request::{has_tree_sitter_query, request_export_method};

pub(super) fn request_prompt_output_writeback_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if request.is_hook_direct_source_read() {
        return None;
    }

    match request.method {
        ClientMethod::Search if is_replayable_search_prompt_output(&request.forwarded_args) => {
            request_export_method(request)
        }
        ClientMethod::Query if is_replayable_query_prompt_output(&request.forwarded_args) => {
            request_export_method(request)
        }
        _ => None,
    }
}

pub(super) fn request_search_packet_writeback_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Search
        || request
            .forwarded_args
            .iter()
            .any(|arg| arg == "items" || arg == "ingest" || arg == "--code" || arg == "--json")
        || !(is_seed_search_without_code(&request.forwarded_args)
            || is_dependency_search(&request.forwarded_args))
    {
        return None;
    }
    request_export_method(request)
}

pub(super) fn request_search_packet_provider_export_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if is_prime_seed_search(&request.forwarded_args) {
        return None;
    }
    if !is_search_packet_seed_search(&request.forwarded_args)
        && !is_dependency_search(&request.forwarded_args)
    {
        return None;
    }
    request_search_packet_writeback_method(request)
}

pub(super) fn request_query_packet_writeback_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Query
        || request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code")
        || has_selector_query(&request.forwarded_args)
    {
        return None;
    }
    let export_method = request_export_method(request)?;
    if export_method.as_str() == "query/owner-items" {
        Some(export_method)
    } else {
        None
    }
}

pub(super) fn request_syntax_query_writeback_method(
    request: &ClientRequest,
) -> Option<CacheExportMethod> {
    if request.method != ClientMethod::Query
        || request
            .forwarded_args
            .iter()
            .any(|arg| arg == "--json" || arg == "--code")
        || !has_tree_sitter_query(&request.forwarded_args)
    {
        return None;
    }
    let export_method = request_export_method(request)?;
    if export_method.as_str() == "query/tree-sitter" {
        Some(export_method)
    } else {
        None
    }
}

pub(super) fn insert_json_flag_before_project_root(args: &mut Vec<String>) {
    let insert_at = if args.last().is_some_and(|arg| arg == ".") {
        args.len().saturating_sub(1)
    } else {
        args.len()
    };
    args.insert(insert_at, "--json".to_string());
}

fn is_replayable_search_prompt_output(args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "--code" || arg == "--json") {
        return false;
    }
    is_seed_search_without_code(args) || is_owner_items_search(args) || is_dependency_search(args)
}

fn is_replayable_query_prompt_output(args: &[String]) -> bool {
    !args.iter().any(|arg| arg == "--json")
        && !has_code_output(args)
        && has_selector_query(args)
        && !has_tree_sitter_query(args)
}

fn has_code_output(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--code")
}

fn has_selector_query(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--selector")
        || args.iter().any(|arg| arg.starts_with("--selector="))
}

fn is_dependency_search(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| arg == "dependency" || arg == "deps")
}

fn is_owner_items_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "owner") && args.iter().any(|arg| arg == "items")
}

fn is_search_packet_seed_search(args: &[String]) -> bool {
    args.first()
        .is_some_and(|arg| arg == "fzf" || arg == "pipe")
        && is_seed_search_without_code(args)
}

fn is_seed_search_without_code(args: &[String]) -> bool {
    if args
        .iter()
        .any(|arg| arg == "items" || arg == "--code" || arg == "--json")
    {
        return false;
    }
    args.windows(2)
        .any(|window| window[0] == "--view" && window[1] == "seeds")
        || args.iter().any(|arg| arg == "--view=seeds")
}

fn is_prime_seed_search(args: &[String]) -> bool {
    args.first().is_some_and(|arg| arg == "prime") && is_seed_search_without_code(args)
}

#[cfg(test)]
#[path = "../../tests/unit/cache_cli/writeback_request.rs"]
mod writeback_request_tests;
