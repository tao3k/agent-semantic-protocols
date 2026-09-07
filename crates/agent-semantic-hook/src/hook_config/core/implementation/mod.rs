// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Compiles hook configuration into runtime matching rules.

pub(super) mod action_match;
pub(super) use action_match as action_match_facade;
pub(super) mod argv_source;
pub(in crate::hook_config) mod compiled_rule;
mod dispatch_fields;
mod materialized_decision_shards;
mod profile_provider_projection;

pub use compiled_rule::ClientHookConfig;
pub use compiled_rule::DurableHookConfigArtifact;
pub(in crate::hook_config) use compiled_rule::compile_config;
pub(in crate::hook_config) use compiled_rule::compile_config_with_executable_capabilities;
pub use materialized_decision_shards::MaterializedDecisionShards;
#[cfg(test)]
#[path = "../../../../tests/unit/hook_config_core.rs"]
mod tests;
