// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

//! Process-safe scenario execution and typed acceptance helpers for ASP Hooks.

mod codex_rollout;
#[cfg(feature = "compiler")]
pub mod hook_scenarios;
#[cfg(feature = "compiler")]
pub mod installed_publication;
mod runtime;

pub use codex_rollout::CodexPreToolContext;
pub use codex_rollout::project_codex_exec_command_calls;
pub use runtime::DEFAULT_HOOK_TIMEOUT;
pub use runtime::DEFAULT_SCENARIO_CONCURRENCY;
pub use runtime::HookProcessSpec;
pub use runtime::HookScenario;
pub use runtime::HookScenarioReceipt;
pub use runtime::HookTestKitError;
#[cfg(feature = "compiler")]
pub use runtime::classify_codex_plugin_scenario;
#[cfg(feature = "compiler")]
pub use runtime::classify_hook_scenario;
pub use runtime::run_hook_process;
pub use runtime::run_process_scenarios;
pub use runtime::run_scenarios_with;
