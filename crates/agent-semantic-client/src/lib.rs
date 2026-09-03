#![deny(dead_code)]

//! Public `asp` client, CLI, and Runtime Server lifecycle surface.

extern crate self as agent_semantic_client;

mod cache_cli;
pub use cache_cli::{project_registry_clean_clap_command, project_registry_gc_clap_command};
pub mod cli;
mod cli_args;
mod client_cli;
mod command;
mod hook_break_glass;
pub(crate) mod server;
pub use agent_semantic_context_product as context_product_state;

pub mod agent_session_lifecycle_projection;
pub mod exact_projection;
pub mod graph;
mod language_command;
pub mod provider_runtime_storage;
mod runtime_language_client;
#[cfg(test)]
#[path = "../tests/unit/runtime_language_client.rs"]
mod runtime_language_client_tests;
mod search_history;
mod state_cli;
mod syntax_query_preflight;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
mod tools_cli;
pub use language_command::{
    LanguageCommandApplication, LanguageCommandClient, LanguageCommandDispatchFuture,
    LanguageCommandFuture, LanguageCommandOperation, LanguageCommandRequest,
    LanguageCommandResponse, RuntimeLanguageCommandApplication, RuntimeLanguageCommandClient,
    execute_language_command,
};
pub use runtime_language_client::{AspClient, ClientBackpressureProbeReceipt};

pub mod cli_failure;
pub use command::protocol_binary::{
    publish_runtime_server_artifact, published_runtime_server_artifact_digest,
};
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;

pub use agent_semantic_client_core::LanguageId;
pub use agent_semantic_client_server::{
    ProviderProjectResolution, ProviderProjectResolutionCandidates,
    ProviderProjectResolutionPolicyExclusion, encode_provider_project_resolution_request,
    project_resolution_from_stdout, provider_project_resolution_candidates,
};
pub use cli::{run_cli_args, run_cli_from_env};
pub use client_cli::{
    run_cli_args as run_client_cli_args, run_cli_from_env as run_client_cli_from_env,
};
pub use syntax_query_preflight::validate_syntax_query_request as validate_client_syntax_query_request;

#[cfg(test)]
#[path = "../tests/unit/cli_args.rs"]
mod cli_args_tests;
#[cfg(test)]
#[path = "../tests/unit/client_cli.rs"]
mod client_cli_tests;
pub mod projection_presentation;
#[cfg(test)]
#[path = "../tests/unit/provider_runtime_storage.rs"]
mod provider_runtime_storage_tests;
#[cfg(test)]
#[path = "../tests/unit/search_history.rs"]
mod search_history_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_query_preflight.rs"]
mod syntax_query_preflight_tests;
#[cfg(test)]
#[path = "../tests/unit/tools_cli.rs"]
mod tools_cli_tests;
