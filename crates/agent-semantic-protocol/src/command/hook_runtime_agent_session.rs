//! Agent session routing owns resident child lifecycle decisions for hook-time `asp` commands.

use crate::command::{has_current_agent_session, record_current_session_tool_event};
use agent_semantic_config::agent_route_registry::{
    AgentSessionLifetime, CompiledAgentRoute, compile_agent_route, load_agent_route_registry,
};
use agent_semantic_config::{
    HookClientAgentSessionMessagesConfig, load_asp_project_config_file,
    load_hook_client_config_file, merge_asp_project_hook_config,
};
use agent_semantic_hook::{
    DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID, HOOK_DECISION_SCHEMA_VERSION,
    HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookDecision, ReasonKind,
};
#[path = "hook_runtime_agent_session_identity.rs"]
mod hook_runtime_agent_session_identity;
#[path = "hook_runtime_agent_session_pane.rs"]
mod hook_runtime_agent_session_pane;
#[path = "hook_runtime_agent_session_payload.rs"]
mod hook_runtime_agent_session_payload;
#[path = "hook_runtime_agent_session_presence.rs"]
mod hook_runtime_agent_session_presence;
#[path = "hook_runtime_agent_session_profile.rs"]
mod hook_runtime_agent_session_profile;
#[path = "hook_runtime_agent_session_rollout_topology.rs"]
mod hook_runtime_agent_session_rollout_topology;
#[path = "hook_runtime_agent_session_session_start.rs"]
mod hook_runtime_agent_session_session_start;
#[path = "hook_runtime_agent_session_terminal.rs"]
mod hook_runtime_agent_session_terminal;
#[path = "hook_runtime_agent_session_typed_replacement.rs"]
mod hook_runtime_agent_session_typed_replacement;
use hook_runtime_agent_session_pane::{
    agent_session_allow_decision, agent_session_route_fields, render_agent_session_template,
};
pub(in crate::command::hook_runtime) use hook_runtime_agent_session_payload::payload_command_strings;
use hook_runtime_agent_session_payload::{payload_evidence_ref, string_field};
use hook_runtime_agent_session_profile::{
    append_resident_agent_fields, resident_child_create_action,
};
use hook_runtime_agent_session_session_start::classify_session_start_bootstrap;
pub(super) use hook_runtime_agent_session_session_start::{
    current_session_configured_resident_identity_proof,
    current_session_resident_child_identity_proof, session_matches_resident_agent,
};
pub(super) use hook_runtime_agent_session_terminal::append_terminal_execution_fields;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct AspSessionPolicy {
    resident_route: CompiledAgentRoute,
    messages: HookClientAgentSessionMessagesConfig,
}

impl AspSessionPolicy {
    pub(super) fn enabled(&self) -> bool {
        true
    }

    pub(super) fn resident_child_name(&self) -> &str {
        self.resident_route.session_name.as_str()
    }

    pub(super) fn resident_agent_role(&self) -> &str {
        self.resident_route.route_key.as_str()
    }

    pub(super) fn resident_codex_agent_name(&self) -> &str {
        self.resident_route.platform_host_agent_name.as_str()
    }
}

pub(super) fn load_asp_session_policy(
    config_path: &Path,
    project_root: &Path,
) -> Result<AspSessionPolicy, String> {
    let base = load_hook_client_config_file(config_path)?;
    let project = load_asp_project_config_file(&agent_semantic_hook::project_agent_config_path(
        project_root,
    ))?;
    let merged = merge_asp_project_hook_config(base, project)?;
    AspSessionPolicy::from_agent_route_registry(merged.agent_session_messages)
}

pub(super) fn load_embedded_asp_session_policy(
    project_root: &Path,
) -> Result<AspSessionPolicy, String> {
    let base = agent_semantic_config::default_hook_client_config_file()?;
    let project = load_asp_project_config_file(&agent_semantic_hook::project_agent_config_path(
        project_root,
    ))?;
    let merged = merge_asp_project_hook_config(base, project)?;
    AspSessionPolicy::from_agent_route_registry(merged.agent_session_messages)
}

impl AspSessionPolicy {
    fn from_agent_route_registry(
        messages: HookClientAgentSessionMessagesConfig,
    ) -> Result<Self, String> {
        let registry_path = agent_semantic_runtime::resolve_state_home()?
            .join("agents")
            .join("config.toml");
        let loaded = load_agent_route_registry(&registry_path)?;
        let (route_key, _) = loaded
            .registry
            .agents
            .iter()
            .find(|(_, route)| {
                route.session_lifetime == AgentSessionLifetime::Resident
                    && route.roles.iter().any(|role| role == "explore")
            })
            .ok_or_else(|| {
                format!(
                    "agent route registry {} must define a resident route with role `explore`",
                    registry_path.display()
                )
            })?;
        let resident_route = compile_agent_route(&loaded, route_key, "codex")?;
        Ok(Self {
            resident_route,
            messages,
        })
    }
}

pub(super) fn classify_main_session_asp_exploration(
    project_root: &Path,
    platform: &str,
    event: &str,
    asp_session_policy: &AspSessionPolicy,
    payload: &serde_json::Value,
) -> Result<Option<HookDecision>, String> {
    if !asp_session_policy.enabled() {
        return Ok(None);
    }
    match event {
        "session-start" | "subagent-start" => classify_session_start_bootstrap(
            project_root,
            platform,
            event,
            payload,
            asp_session_policy,
        ),
        "post-tool" => {
            record_post_tool_session_event(project_root, event, payload)?;
            if has_current_agent_session() {
                Ok(Some(agent_session_allow_decision(
                    platform,
                    event,
                    payload,
                    "post-tool-session-event-recorded",
                    "ASP recorded post-tool session activity.",
                )))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn record_post_tool_session_event(
    project_root: &Path,
    event: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let command = payload_command_strings(payload).into_iter().next();
    let evidence_ref = payload_evidence_ref(payload);
    record_current_session_tool_event(
        project_root,
        event,
        command.as_deref(),
        evidence_ref.as_deref(),
    )?;
    Ok(())
}

fn template_value(key: &'static str, value: impl Into<String>) -> (&'static str, String) {
    (key, value.into())
}

fn unix_timestamp() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before unix epoch: {error}"))?;
    i64::try_from(duration.as_secs()).map_err(|error| format!("timestamp overflow: {error}"))
}
