use std::path::Path;

use agent_semantic_client_db::{
    AgentSessionLookupRequest, AgentSessionRegistry, agent_session_unix_timestamp,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_state::{project_session_scope_id, resolved_root_session_id};

pub(super) fn resume_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    let project_id = project_session_scope_id(registry, project_root)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.clone();
    let record = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: args.child_session_id.as_deref(),
        root_session_id: root_session_id.as_deref(),
        name: name.as_deref(),
    })?;
    let now = agent_session_unix_timestamp()?;
    let registry_status = record
        .as_ref()
        .map(|session| session.status.as_str())
        .unwrap_or("missing");
    let routable = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed") && session.is_routable_at(now)
        })
        .unwrap_or(false);
    let next_action = if routable {
        "send-input-to-existing-child"
    } else if record.is_some() {
        "run-agent-session-status-before-replace"
    } else {
        "register-or-create-child"
    };
    let session_id = record
        .as_ref()
        .map(|session| session.session_id.as_str())
        .unwrap_or("");
    let root_session = record
        .as_ref()
        .map(|session| session.root_session_id.as_str())
        .or(root_session_id.as_deref())
        .unwrap_or("");
    let role = record
        .as_ref()
        .map(|session| session.role.as_str())
        .unwrap_or("");
    let model = record
        .as_ref()
        .and_then(|session| session.model.as_deref())
        .unwrap_or("");
    let name = name
        .as_deref()
        .or_else(|| record.as_ref().map(|session| session.name.as_str()))
        .unwrap_or("");
    if routable {
        println!(
            "[agent-session-status] owner=rust name=\"{}\" session=\"{}\" rootSession=\"{}\" registryStatus=\"{}\" routable=true role=\"{}\" model=\"{}\" rolloutActivityStatus=\"agent-active\" nextAction=\"child-activity-running-wait\" db=\"{}\"",
            resume_field(name),
            resume_field(session_id),
            resume_field(root_session),
            resume_field(registry_status),
            resume_field(role),
            resume_field(model),
            resume_field(&registry.db_path().display().to_string())
        );
        return Ok(());
    }
    println!(
        "[agent-session-resume] owner=rust name=\"{}\" session=\"{}\" rootSession=\"{}\" registryStatus=\"{}\" routable={} role=\"{}\" model=\"{}\" nextAction=\"{}\" db=\"{}\"",
        resume_field(name),
        resume_field(session_id),
        resume_field(root_session),
        resume_field(registry_status),
        routable,
        resume_field(role),
        resume_field(model),
        resume_field(next_action),
        resume_field(&registry.db_path().display().to_string())
    );
    Ok(())
}

fn resume_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
