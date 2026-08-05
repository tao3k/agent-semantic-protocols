//! Durable agent session and subagent registry.

#[path = "agent_config_sync.rs"]
mod agent_config_sync;
#[path = "agent_session_registry_args.rs"]
mod agent_session_registry_args;
#[path = "agent_session_registry_bootstrap.rs"]
mod agent_session_registry_bootstrap;
#[path = "agent_session_registry_codex.rs"]
mod agent_session_registry_codex;
#[path = "agent_session_registry_command_parts/mod.rs"]
mod agent_session_registry_command_parts;
#[path = "agent_session_registry_commands.rs"]
mod agent_session_registry_commands;
#[path = "agent_session_registry_control_plane.rs"]
mod agent_session_registry_control_plane;
pub(in crate::command::agent_session_registry) use agent_session_registry_commands::stale_invalid_session_should_be_idle;
#[path = "agent_session_registry_dispatch.rs"]
mod agent_session_registry_dispatch;
#[path = "agent_session_registry_host_capability.rs"]
pub(in crate::command) mod agent_session_registry_host_capability;
pub(super) use agent_session_registry_host_capability::record_subagent_start_target_present;
#[path = "agent_session_registry_lifecycle_audit.rs"]
mod agent_session_registry_lifecycle_audit;
#[path = "agent_session_registry_lifetime.rs"]
mod agent_session_registry_lifetime;
#[path = "agent_session_registry_profile.rs"]
mod agent_session_registry_profile;
#[path = "agent_session_registry_render.rs"]
mod agent_session_registry_render;
#[path = "agent_session_registry_resume.rs"]
mod agent_session_registry_resume;
#[path = "agent_session_registry_rollout_activity.rs"]
mod agent_session_registry_rollout_activity;
#[path = "agent_session_registry_rollout_adopt.rs"]
mod agent_session_registry_rollout_adopt;
#[path = "agent_session_registry_rollout_lookup.rs"]
mod agent_session_registry_rollout_lookup;
#[path = "agent_session_registry_state.rs"]
mod agent_session_registry_state;
pub(crate) use agent_session_registry_state::payload_live_target_resident_identity_proof;
pub(crate) use agent_session_registry_state::payload_live_target_resident_identity_status;
#[path = "agent_session_registry_tool_event.rs"]
mod agent_session_registry_tool_event;
#[path = "agent_session_registry_validation.rs"]
mod agent_session_registry_validation;
pub(crate) use agent_session_registry_validation::{
    rollout_metadata_matches_host_agent_identity, validate_session_profile,
};

use agent_semantic_client_db::AgentSessionRegistry;
use agent_semantic_runtime::AgentSessionValidationReport as SessionValidationReport;
use agent_session_registry_args::{
    SessionArgs, SessionCommand, agent_usage, session_guide, session_usage,
};
use agent_session_registry_codex::run_codex_session_wrapper;
use agent_session_registry_command_parts::{
    close_session, gc_sessions, reconcile_sessions, status_session,
};
use agent_session_registry_commands::{
    lifecycle_audit_session, list_sessions, register_session, show_session, smoke_session,
};
use std::{env, path::PathBuf};

const EMBEDDED_AGENT_ROUTE_REGISTRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../agents/config.toml"
));

fn configured_resident_session_name(requested: Option<&str>) -> Result<String, String> {
    let registry =
        toml::from_str::<agent_semantic_config::agent_route_registry::AgentRouteRegistry>(
            EMBEDDED_AGENT_ROUTE_REGISTRY,
        )
        .map_err(|error| format!("failed to parse embedded agent route registry: {error}"))?;
    if let Some(requested) = requested {
        return registry
            .agents
            .values()
            .find(|route| route.session_name == requested)
            .map(|route| route.session_name.clone())
            .ok_or_else(|| {
                format!("agent session `{requested}` is absent from the route registry")
            });
    }
    let mut explore_routes = registry
        .agents
        .values()
        .filter(|route| route.roles.iter().any(|role| role == "explore"));
    let route = explore_routes
        .next()
        .ok_or_else(|| "agent route registry omitted the explore session".to_string())?;
    if explore_routes.next().is_some() {
        return Err("agent route registry has multiple explore sessions".to_string());
    }
    Ok(route.session_name.clone())
}

pub(crate) use agent_session_registry_state::{
    ResidentChildIdentityProof, codex_transcript_resident_child_identity,
    current_registered_session, current_resident_child_identity_proof, current_root_session_id,
    has_current_agent_session, registered_resident_session_for_root, registered_root_session_id,
};
pub(crate) use agent_session_registry_tool_event::record_current_session_tool_event;

pub(crate) fn run_agent_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("session") => run_agent_session_command(&args[1..]),
        Some("config") => agent_config_sync::run_agent_config_command(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            println!("{}", agent_usage());
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown agent command `{command}`\n{}",
            agent_usage()
        )),
    }
}

use self::agent_session_registry_state::{project_session_scope_id, resolved_root_session_id};

pub(crate) fn run_agent_session_command(args: &[String]) -> Result<(), String> {
    let args = SessionArgs::parse(args)?;
    if args.help {
        println!("{}", session_usage());
        return Ok(());
    }
    if args.guide {
        println!("{}", session_guide(args.command)?);
        return Ok(());
    }

    let project_root =
        env::current_dir().map_err(|error| format!("failed to read current directory: {error}"))?;
    let state_home = std::env::var_os("ASP_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".agent-semantic-protocols"))
        })
        .ok_or_else(|| "ASP_STATE_HOME cannot be resolved for runtime repair".to_owned())?;
    let plan = super::protocol_binary::ProtocolBinaryInstallPlan::capture(
        state_home.join("runtime/artifacts"),
    )?;
    super::protocol_binary::ensure_protocol_binary_installed(&plan)?;
    let started = std::time::Instant::now();
    ensure_agent_session_runtime_server(started.into(), &project_root)?;
    let registry = AgentSessionRegistry::open_runtime_project_proxy(&project_root)?
        .ok_or_else(|| {
            "registryStatus=missing registryWriteStatus=not-attempted; Runtime Server registry projection is unavailable"
                .to_string()
        })?;

    match args.command {
        SessionCommand::Bootstrap => {
            agent_session_registry_bootstrap::bootstrap_session(&registry, &args, &project_root)
        }
        SessionCommand::ObserveHostCapability => {
            agent_session_registry_host_capability::observe_host_capability(&registry, &args)
        }
        SessionCommand::ObserveHostTree => {
            agent_session_registry_host_capability::observe_host_tree(&registry, &args)
        }
        SessionCommand::ObserveHostAck => {
            agent_session_registry_host_capability::observe_host_ack(&registry, &args)
        }
        SessionCommand::DispatchClaim => {
            agent_session_registry_dispatch::claim_dispatch(&registry, &args)
        }
        SessionCommand::DispatchExecute => {
            agent_session_registry_dispatch::execute_dispatch(&registry, &args)
        }
        SessionCommand::DispatchComplete => {
            agent_session_registry_dispatch::complete_dispatch(&registry, &args)
        }
        SessionCommand::DispatchMarkOrphaned => {
            agent_session_registry_dispatch::mark_dispatch_orphaned(&registry, &args)
        }
        SessionCommand::Register => register_session(&registry, &args),
        SessionCommand::List => list_sessions(&registry, &args),
        SessionCommand::Show => show_session(&registry, &args),
        SessionCommand::Status => status_session(&registry, &args, &project_root),
        SessionCommand::LifecycleAudit => lifecycle_audit_session(&registry, &args),
        SessionCommand::ControlPlaneRefresh => {
            agent_session_registry_control_plane::refresh_control_plane(&args, &project_root)
        }
        SessionCommand::ControlPlaneShow => {
            agent_session_registry_control_plane::show_control_plane(&args, &project_root)
        }
        SessionCommand::Smoke => smoke_session(&registry, &args),
        SessionCommand::Close => close_session(&registry, &args),
        SessionCommand::Gc => gc_sessions(&registry, &args),
        SessionCommand::Reconcile => reconcile_sessions(&registry, &args),
        SessionCommand::Resume if should_render_resume_status(&args) => {
            agent_session_registry_resume::resume_session(&registry, &args, &project_root)
        }
        SessionCommand::Resume => run_codex_session_wrapper(&registry, &args, "resume", false),
        SessionCommand::Fork => run_codex_session_wrapper(&registry, &args, "fork", false),
        SessionCommand::Archive => run_codex_session_wrapper(&registry, &args, "archive", false),
        SessionCommand::Delete => run_codex_session_wrapper(&registry, &args, "delete", true),
        SessionCommand::Unarchive => {
            run_codex_session_wrapper(&registry, &args, "unarchive", false)
        }
    }
}

fn ensure_agent_session_runtime_server(
    _started: tokio::time::Instant,
    project_root: &std::path::Path,
) -> Result<(), String> {
    let state = agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?;
    state.ensure_minimal_layout()?;
    let healthy = crate::server::runtime_server::block_on_agent_facing_runtime_server_client(
        tokio::time::Instant::now(),
        "agent-session",
        "runtime-server-healthcheck",
        project_root,
        async {
            Ok(
                crate::server::runtime_server::probe_healthy_runtime_server_at(&state.state_home)
                    .await
                    .unwrap_or(false),
            )
        },
    )?;
    if healthy {
        return Ok(());
    }
    crate::server::runtime_server::block_on_runtime_server_supervisor_client(
        "agent-session",
        "runtime-server-reconcile",
        async {
            crate::server::runtime_server_supervisor::reconcile_healthy_runtime_server(
                &state.state_home,
            )
            .await
            .map(|_| ())
        },
    )
}

fn should_render_resume_status(args: &SessionArgs) -> bool {
    if args.json {
        return true;
    }
    agent_platform_session_active() || !std::io::IsTerminal::is_terminal(&std::io::stdin())
}

fn agent_platform_session_active() -> bool {
    if env::var_os("ASP_NO_AGENT_PLATFORM").is_some() {
        return false;
    }
    [
        "CODEX_THREAD_ID",
        "CODEX_PARENT_THREAD_ID",
        "CLAUDE_SESSION_ID",
        "CLAUDE_CODE_SESSION_ID",
        "AGENT_SESSION_ID",
        "AGENT_PLATFORM_SESSION_ID",
    ]
    .into_iter()
    .any(|name| env::var_os(name).is_some_and(|value| !value.is_empty()))
}

pub(crate) fn active_platform() -> Option<&'static str> {
    if env::var_os("CODEX_THREAD_ID").is_some() {
        return Some("codex");
    }
    if env::var_os("CLAUDE_CODE_SESSION_ID").is_some()
        || env::var_os("CLAUDE_CODE_REMOTE_SESSION_ID").is_some()
    {
        return Some("claude-code");
    }
    None
}

pub(super) fn codex_home() -> PathBuf {
    env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .unwrap_or_else(|| PathBuf::from(".codex"))
}

pub(crate) struct SessionRoleDefaults {
    pub(crate) roles: Vec<String>,
    pub(crate) permissions: Vec<String>,
}

pub(crate) fn session_role_defaults_for_session_name(
    name: &str,
) -> Result<SessionRoleDefaults, String> {
    let config = agent_semantic_config::default_hook_client_config_file()?;
    let Some(agent) = config
        .agents
        .resident_agents
        .iter()
        .find(|agent| agent.name == name)
    else {
        return Ok(SessionRoleDefaults {
            roles: Vec::new(),
            permissions: Vec::new(),
        });
    };
    Ok(SessionRoleDefaults {
        roles: agent.roles.clone(),
        permissions: agent.permissions.clone(),
    })
}

pub(crate) fn normalize_session_roles(roles: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = roles.to_vec();
    normalized.sort();
    normalized.dedup();
    for role in &normalized {
        if !matches!(
            role.as_str(),
            "subagent" | "search" | "testing" | "build" | "checkpoint"
        ) {
            return Err(format!(
                "unknown session role `{role}`; expected one of subagent, search, testing, build, checkpoint"
            ));
        }
    }
    if normalized.is_empty() {
        return Err(
            "agent session register requires --roles or configured agents.residentAgents[].roles"
                .to_string(),
        );
    }
    Ok(normalized)
}

pub(crate) fn session_permissions_for_roles(roles: &[String]) -> Vec<String> {
    let mut permissions = Vec::new();
    if roles.iter().any(|role| role == "search") {
        permissions.push("read-only".to_string());
    }
    if roles
        .iter()
        .any(|role| matches!(role.as_str(), "testing" | "build"))
    {
        permissions.push("workspace-write".to_string());
    }
    permissions.sort();
    permissions.dedup();
    permissions
}

pub(crate) fn normalize_session_permissions(permissions: &[String]) -> Result<Vec<String>, String> {
    let mut normalized = permissions.to_vec();
    normalized.sort();
    normalized.dedup();
    for permission in &normalized {
        if !matches!(
            permission.as_str(),
            "read-only" | "workspace-write" | "danger-full-access"
        ) {
            return Err(format!(
                "unknown session permission `{permission}`; expected one of read-only, workspace-write, danger-full-access"
            ));
        }
    }
    Ok(normalized)
}

pub(crate) fn normalized_metadata(
    metadata_json: Option<&str>,
    validation: &SessionValidationReport,
) -> Result<String, String> {
    let mut value = match metadata_json {
        Some(metadata_json) if !metadata_json.trim().is_empty() => {
            serde_json::from_str::<serde_json::Value>(metadata_json)
                .map_err(|error| format!("failed to parse session metadata JSON: {error}"))?
        }
        _ => serde_json::json!({}),
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| "session metadata must be a JSON object".to_string())?;
    let sandbox_verification_status =
        sandbox_verification_status(validation.expected_sandbox(), validation.actual_sandbox());
    object.insert(
        "validationStatus".to_string(),
        serde_json::Value::String(validation.status().as_str().to_string()),
    );
    object.insert(
        "validationReason".to_string(),
        serde_json::Value::String(validation.reason().to_string()),
    );
    object.insert(
        "validation".to_string(),
        serde_json::json!({
            "status": validation.status(),
            "reason": validation.reason(),
            "configPath": validation.config_path(),
            "rolloutPath": validation.rollout_path(),
            "expectedRootSessionId": validation.expected_root_session_id(),
            "actualRootSessionId": validation.actual_root_session_id(),
            "expectedParentThreadId": validation.expected_parent_thread_id(),
            "actualParentThreadId": validation.actual_parent_thread_id(),
            "expectedAgentPath": validation.expected_agent_path(),
            "actualAgentPath": validation.actual_agent_path(),
            "expectedRole": validation.expected_role(),
            "actualRole": validation.actual_role(),
            "expectedModel": validation.expected_model(),
            "actualModel": validation.actual_model(),
            "expectedSandbox": validation.expected_sandbox(),
            "actualSandbox": validation.actual_sandbox(),
            "sandboxVerificationStatus": sandbox_verification_status,
            "sandboxPolicy": "warning-only-host-inherited",
            "sandboxAffectsReady": false,
        }),
    );
    serde_json::to_string(&value)
        .map_err(|error| format!("failed to serialize normalized session metadata: {error}"))
}

fn sandbox_verification_status(expected: Option<&str>, actual: Option<&str>) -> &'static str {
    if expected == actual {
        "matched"
    } else {
        "host-inherited-drift-warning"
    }
}

#[cfg(test)]
#[path = "../../tests/unit/agent_session_registry_sandbox.rs"]
mod sandbox_verification_status_tests;

pub(in crate::command::agent_session_registry) fn normalized_metadata_with_roles(
    metadata_json: Option<&str>,
    validation: &SessionValidationReport,
    roles: &[String],
    permissions: &[String],
) -> Result<String, String> {
    let metadata = normalized_metadata(metadata_json, validation)?;
    let mut value = serde_json::from_str::<serde_json::Value>(&metadata)
        .map_err(|error| format!("failed to parse normalized session metadata: {error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "normalized session metadata must be a JSON object".to_string())?;
    object.insert(
        "roles".to_string(),
        serde_json::Value::Array(
            roles
                .iter()
                .map(|role| serde_json::Value::String(role.clone()))
                .collect(),
        ),
    );
    object.insert(
        "permissions".to_string(),
        serde_json::Value::Array(
            permissions
                .iter()
                .map(|permission| serde_json::Value::String(permission.clone()))
                .collect(),
        ),
    );
    serde_json::to_string(&value)
        .map_err(|error| format!("failed to encode session metadata: {error}"))
}
