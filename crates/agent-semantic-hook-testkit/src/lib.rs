//! Process-safe scenario execution and typed acceptance helpers for ASP Hooks.

#[cfg(feature = "compiler")]
pub mod hook_scenarios;
#[cfg(feature = "compiler")]
pub mod installed_publication;
mod runtime;

pub use runtime::{
    DEFAULT_HOOK_TIMEOUT, DEFAULT_SCENARIO_CONCURRENCY, HookProcessRecoveryReceipt,
    HookProcessSpec, HookScenario, HookScenarioReceipt, HookTestKitError, run_hook_process,
    run_process_entry_no_agent_recovery_probe, run_process_scenarios, run_scenarios_with,
};
#[cfg(feature = "compiler")]
pub use runtime::{classify_codex_plugin_scenario, classify_hook_scenario};
