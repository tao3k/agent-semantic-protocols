#![deny(dead_code)]

//! Agent-facing `asp` client command surface.

mod cache_cli;
pub use cache_cli::{project_registry_clean_clap_command, project_registry_gc_clap_command};
pub mod cli;
mod cli_args;
pub mod provider_runtime_storage;
mod search_history;
pub mod source_index;
mod syntax_query_preflight;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
mod tools_cli;

pub use agent_semantic_client_core::LanguageId;
pub use agent_semantic_client_server::{
    ProviderProjectResolution, ProviderProjectResolutionCandidates,
    ProviderProjectResolutionPolicyExclusion, encode_provider_project_resolution_request,
    project_resolution_from_stdout, provider_project_resolution_candidates,
};
pub use agent_semantic_runtime::{
    LanguageOwnerItemsAttempt, LanguageOwnerItemsDispatchPlan, language_owner_items_workspace_root,
    language_owner_path_exists, run_language_owner_items_dispatch_plan,
};
pub use cli::{run_cli_args, run_cli_from_env};
pub use source_index::{
    SourceIndexCandidate, SourceIndexLookupRequest, SourceIndexLookupResult,
    SourceIndexLookupState, SourceIndexRefreshReport, SourceIndexSourceKind,
    lookup_search_pipe_source_index_for_language, lookup_source_index,
    lookup_source_index_for_language,
};
pub use syntax_query_preflight::validate_syntax_query_request as validate_client_syntax_query_request;

#[cfg(test)]
#[path = "../tests/unit/cli_args.rs"]
mod cli_args_tests;
#[cfg(test)]
#[path = "../tests/unit/cli.rs"]
mod cli_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_runtime_storage.rs"]
mod provider_runtime_storage_tests;
#[cfg(test)]
#[path = "../tests/unit/search_history.rs"]
mod search_history_tests;
#[cfg(test)]
#[path = "../tests/unit/source_index_lookup.rs"]
mod source_index_lookup_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_query_preflight.rs"]
mod syntax_query_preflight_tests;
#[cfg(test)]
#[path = "../tests/unit/tools_cli.rs"]
mod tools_cli_tests;
