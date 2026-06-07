#![deny(dead_code)]

//! Agent-facing `asp` client command surface.

mod cache_cli;
mod cache_replay;
pub mod cli;
mod cli_args;
mod compact_mode;
mod provider_method;
mod search_history;
mod syntax_query_preflight;
mod syntax_receipt;
#[cfg(test)]
#[path = "../tests/unit/support.rs"]
mod test_support;
mod tools_cli;

pub use agent_semantic_client_core::LanguageId;
pub use cli::{run_cli_args, run_cli_from_env};

#[cfg(test)]
#[path = "../tests/unit/cache_cli/command.rs"]
mod cache_cli_command_tests;
#[cfg(test)]
#[path = "../tests/unit/cache_cli/probe.rs"]
mod cache_cli_probe_tests;

#[cfg(test)]
#[path = "../tests/unit/cache_replay/row_replay.rs"]
mod cache_replay_row_replay_tests;
#[cfg(test)]
#[path = "../tests/unit/cache_replay/structured_evidence.rs"]
mod cache_replay_structured_evidence_tests;
#[cfg(test)]
#[path = "../tests/unit/cache_replay.rs"]
mod cache_replay_tests;

#[cfg(test)]
#[path = "../tests/unit/cli_args.rs"]
mod cli_args_tests;
#[cfg(test)]
#[path = "../tests/unit/compact_mode.rs"]
mod compact_mode_tests;
#[cfg(test)]
#[path = "../tests/unit/provider_method.rs"]
mod provider_method_tests;
#[cfg(test)]
#[path = "../tests/unit/search_history.rs"]
mod search_history_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_query_preflight.rs"]
mod syntax_query_preflight_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_receipt.rs"]
mod syntax_receipt_tests;
#[cfg(test)]
#[path = "../tests/unit/tools_cli.rs"]
mod tools_cli_tests;
