//! Agent-facing `asp` client command surface.

mod cache_cli;
mod cache_replay;
pub mod cli;
mod syntax_receipt;

pub use agent_semantic_client_core::LanguageId;
pub use cli::{run_cli_args, run_cli_from_env};

#[cfg(test)]
#[path = "../tests/unit/cache_replay.rs"]
mod cache_replay_tests;

#[cfg(test)]
#[path = "../tests/unit/syntax_receipt.rs"]
mod syntax_receipt_tests;
