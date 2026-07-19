use agent_semantic_client_db::agent_session_registry::interactive_loop::AgentSessionHostRequirement;
use agent_semantic_client_db::{AgentSessionRecord, AgentSessionRegistry};

pub(super) fn reject_resident_child_bootstrap(
    _registry: &AgentSessionRegistry,
    _project_id: &str,
    name: &str,
) -> Result<(), String> {
    let project_root = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let runtime_session = agent_semantic_runtime::current_agent_runtime_session();
    let non_root_session = runtime_session.as_ref().is_some_and(|session| {
        crate::command::agent_session_registry_state::current_root_session_id()
            .is_some_and(|root_session_id| root_session_id != session.id)
    });
    if non_root_session
        || crate::command::agent_session_registry_state::current_resident_child_identity_proof(
            &project_root,
            name,
            "",
        )?
        .is_some()
    {
        let session_id = runtime_session
            .map(|session| session.id)
            .unwrap_or_else(|| "<unknown>".to_string());
        return Err(format!(
            "bootstrap-owner-main-session-only: registered resident child session `{session_id}` must use parser-owned ASP query/search directly and return its receipt; do not enter or execute the lifecycle bootstrap from the child."
        ));
    }
    Ok(())
}

pub(in crate::command::agent_session_registry) fn codex_spawn_agent_metadata_capability()
-> &'static str {
    let config_path = crate::command::sync::codex_home().join("config.toml");
    let Ok(content) = std::fs::read_to_string(config_path) else {
        return "unknown";
    };
    let Ok(config) = toml::from_str::<toml::Value>(&content) else {
        return "unknown";
    };
    match config
        .get("features")
        .and_then(|features| features.get("multi_agent_v2"))
        .and_then(|multi_agent_v2| multi_agent_v2.get("hide_spawn_agent_metadata"))
        .and_then(toml::Value::as_bool)
    {
        Some(true) => "hidden-by-config",
        Some(false) => "visible-agent-type",
        None => "unknown",
    }
}

pub(super) fn platform_native_create_action(
    requirement: &AgentSessionHostRequirement<'_>,
) -> String {
    match requirement.platform {
        "codex" => format!(
            "use a native collaboration spawn surface that explicitly exposes `agent_type`; set `agent_type={}`, `task_name={}`, and `fork_turns=none`, then let Codex apply the auto-loaded TOML profile and let SubagentStart capture the native identity. `agent_type` selects the registered role; `task_name` reserves the canonical /root/{} path; natural-language task payload does not select the role. If `agent_type` is unavailable, report `host-agent-type-unavailable` without spawning a generic child",
            requirement.managed_agent_kind,
            requirement.managed_agent_kind,
            requirement.managed_agent_kind,
        ),
        _ => format!(
            "use the detected platform's managed-agent creation action for {}; let the platform lifecycle-start event capture native identity",
            requirement.managed_agent_kind
        ),
    }
}

pub(super) fn canonical_resident_target(requirement: &AgentSessionHostRequirement<'_>) -> String {
    if requirement.platform == "codex" {
        format!("/root/{}", requirement.managed_agent_kind)
    } else {
        requirement.resident_child_name.to_string()
    }
}

pub(super) fn registry_record_routable(
    record: &AgentSessionRecord,
    current_root_session_id: Option<&str>,
    fresh_host_transport_verified: bool,
    now: i64,
) -> bool {
    !matches!(record.status.as_str(), "archived" | "closed")
        && current_root_session_id.is_some_and(|root| {
            agent_semantic_client_db::agent_session_message_target_is_currently_routable(
                record,
                root,
                fresh_host_transport_verified,
                now,
            )
        })
}
