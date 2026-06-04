//! Cache request classification helpers.

use agent_semantic_client_core::{
    CacheExportMethod, ClientMethod, ClientRequest, ProviderRegistrySnapshot, ResolvedProvider,
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
    } else {
        "query/owner-items".to_string()
    }
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
