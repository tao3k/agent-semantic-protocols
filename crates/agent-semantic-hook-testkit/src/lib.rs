//! Process-safe scenario execution and typed acceptance helpers for ASP Hooks.

#[cfg(feature = "compiler")]
pub mod hook_scenarios;
#[cfg(feature = "compiler")]
pub mod installed_publication;
mod runtime;

pub use runtime::DEFAULT_HOOK_TIMEOUT;
pub use runtime::DEFAULT_SCENARIO_CONCURRENCY;
pub use runtime::HookProcessRecoveryReceipt;
pub use runtime::HookProcessSpec;
pub use runtime::HookScenario;
pub use runtime::HookScenarioReceipt;
pub use runtime::HookTestKitError;
#[cfg(feature = "compiler")]
pub use runtime::classify_codex_plugin_scenario;
#[cfg(feature = "compiler")]
pub use runtime::classify_hook_scenario;
pub use runtime::run_hook_process;
pub use runtime::run_process_entry_no_agent_recovery_probe;
pub use runtime::run_process_scenarios;
pub use runtime::run_scenarios_with;
