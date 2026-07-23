//! Compiles hook configuration into runtime matching rules.

pub(super) mod action_match;
pub(super) mod argv_source;
pub(in crate::hook_config) mod compiled_rule;

pub use compiled_rule::ClientHookConfig;
pub(in crate::hook_config) use compiled_rule::compile_config;
#[cfg(test)]
#[path = "../../../../tests/unit/hook_config_core.rs"]
mod tests;
