#[path = "client_hook_claude_smoke/activation_fixture.rs"]
mod activation_fixture;
#[path = "client_hook_claude_smoke/claude_platform.rs"]
mod claude_platform;
#[path = "client_hook_claude_smoke/codex_session.rs"]
mod codex_session;
#[path = "client_hook_claude_smoke/install.rs"]
mod install;
#[path = "client_hook_claude_smoke/lifecycle_restart.rs"]
mod lifecycle_restart;
#[path = "client_hook_claude_smoke/rollout_fixture.rs"]
mod rollout_fixture;
#[path = "client_hook_claude_smoke/session_policy.rs"]
mod session_policy;
#[path = "client_hook_claude_smoke/support.rs"]
mod support;
#[path = "client_hook_claude_smoke/unmanaged_bootstrap.rs"]
mod unmanaged_bootstrap;

pub(super) use rollout_fixture::write_codex_asp_explore_rollout;
pub(super) use support::{
    claude_fixture, codex_asp_query_payload, force_activation_project_root_to_hook_state,
    install_claude_hooks, install_codex_hooks, prepend_path, register_asp_explore_session,
    register_expired_asp_explore_session, run_claude_pre_tool_decision,
    run_codex_hook_decision_with_env, run_codex_pre_tool_decision,
    run_codex_pre_tool_decision_with_env, show_agent_session_json,
};
