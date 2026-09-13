#![deny(dead_code)]
// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Public `asp` client, CLI, and Runtime Server lifecycle surface.

extern crate self as agent_semantic_client;

mod cache_cleanup_service;
mod cache_cli;
pub use cache_cli::cache_clean_clap_command;
pub mod cli;
mod cli_args;
mod client_cli;
mod command;
mod hook_break_glass;
pub(crate) mod server;
pub use agent_semantic_context_product as context_product_state;

pub mod agent_session_lifecycle_projection;
pub mod exact_projection;
mod language_command;
pub mod provider_runtime_storage;
mod runtime_language_client;
#[cfg(test)]
#[path = "../tests/unit/runtime_language_client.rs"]
mod runtime_language_client_tests;
mod runtime_language_response_decoders;
mod runtime_language_session_registry;
mod state_cli;
mod state_service;
mod syntax_query_preflight;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
mod tools_cli;
pub use language_command::LanguageCommandApplication;
pub use language_command::LanguageCommandClient;
pub use language_command::LanguageCommandDispatchFuture;
pub use language_command::LanguageCommandFuture;
pub use language_command::LanguageCommandOperation;
pub use language_command::LanguageCommandRequest;
pub use language_command::LanguageCommandResponse;
pub use language_command::RuntimeLanguageCommandApplication;
pub use language_command::RuntimeLanguageCommandClient;
pub use language_command::execute_language_command;
pub use runtime_language_client::ClientBackpressureProbeReceipt;
pub use runtime_language_client::{AspClient, AspClientRuntimeHandoff};

pub mod cli_failure;
#[doc(hidden)]
pub use state_cli::run_binary_from_env;
pub(crate) mod codex;

pub use agent_semantic_client_core::LanguageId;
pub use agent_semantic_client_server::ProviderProjectResolution;
pub use agent_semantic_client_server::ProviderProjectResolutionCandidates;
pub use agent_semantic_client_server::ProviderProjectResolutionPolicyExclusion;
pub use agent_semantic_client_server::encode_provider_project_resolution_request;
pub use agent_semantic_client_server::project_resolution_from_stdout;
pub use agent_semantic_client_server::provider_project_resolution_candidates;
pub use cli::run_cli_args;
pub use cli::run_cli_from_env;
pub use client_cli::run_cli_args as run_client_cli_args;
pub use client_cli::run_cli_from_env as run_client_cli_from_env;
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
#[path = "../tests/unit/syntax_query_preflight.rs"]
mod syntax_query_preflight_tests;
#[cfg(test)]
#[path = "../tests/unit/tools_cli.rs"]
mod tools_cli_tests;
