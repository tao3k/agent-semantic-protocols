//! Agent-facing `asp` client command surface.

mod cache_cli;
mod cache_replay;
pub mod cli;
mod syntax_query_preflight;
mod syntax_receipt;

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
#[path = "../tests/unit/cache_replay.rs"]
mod cache_replay_tests;

#[cfg(test)]
#[path = "../tests/unit/syntax_query_preflight.rs"]
mod syntax_query_preflight_tests;
#[cfg(test)]
#[path = "../tests/unit/syntax_receipt.rs"]
mod syntax_receipt_tests;
