//! Durable agent session and subagent registry.

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
    expected_model_for_session_profile, expected_reasoning_effort_for_session_profile,
    rollout_metadata_matches_managed_agent_profile, validate_session_profile,
};

use agent_semantic_client_db::AgentSessionRegistry;
use agent_semantic_config::codex_agent_projection::{
    update_asp_codex_agent_source_and_symlink_projection,
    update_asp_codex_agent_sources_and_symlink_projections, write_codex_dynamic_model,
    write_codex_dynamic_model_for_session,
};
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
use agent_session_registry_state::open_or_create_default_registry;
use std::{env, path::PathBuf};

pub(crate) use agent_session_registry_state::{
    ResidentChildIdentityProof, codex_transcript_resident_child_identity, current_agent_session_id,
    current_registered_session, current_registered_session_identity,
    current_resident_child_identity_proof, current_root_session_id, has_current_agent_session,
    registered_resident_session_for_root, registered_root_session_id,
};
pub(crate) use agent_session_registry_tool_event::record_current_session_tool_event;

pub(crate) fn run_agent_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("session") => run_agent_session_command(&args[1..]),
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
    let projection_only = matches!(
        &args.command,
        SessionCommand::Bootstrap | SessionCommand::Show | SessionCommand::Status
    );
    let registry = match (projection_only, args.state_root.as_deref()) {
        (true, Some(state_root)) => {
            let state_root =
                AgentSessionRegistry::resolve_state_root_override(&project_root, state_root);
            AgentSessionRegistry::open_existing_state_root_read_only(state_root)?.ok_or_else(
                || {
                    "registryStatus=missing registryWriteStatus=not-attempted; run `asp sync` before resident lifecycle projection"
                        .to_string()
                },
            )?
        }
        (true, None) => AgentSessionRegistry::open_existing_project_read_only(&project_root)?
            .ok_or_else(|| {
                "registryStatus=missing registryWriteStatus=not-attempted; run `asp sync` before resident lifecycle projection"
                    .to_string()
            })?,
        (false, Some(state_root)) => {
            let state_root =
                AgentSessionRegistry::resolve_state_root_override(&project_root, state_root);
            AgentSessionRegistry::open_or_create_state_root(state_root)?
        }
        (false, None) => open_or_create_default_registry(&project_root)?,
    };

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
        SessionCommand::SwitchModel => switch_model(&args),
    }
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

fn switch_model(args: &SessionArgs) -> Result<(), String> {
    let model = args
        .model
        .as_deref()
        .ok_or_else(|| "agent session switch-model requires --model".to_string())?;
    let platform = active_platform()
        .ok_or_else(|| "failed to detect active agent platform session".to_string())?;
    match platform {
        "codex" => switch_codex_model(model, args),
        other => Err(format!(
            "agent session switch-model does not support platform `{other}`"
        )),
    }
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

fn switch_codex_model(model: &str, args: &SessionArgs) -> Result<(), String> {
    let agents_config_path = asp_agents_config_path()?;
    let mut updated_agent_configs = Vec::new();
    let asp_agents_dir = agents_config_path
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", agents_config_path.display()))?;
    let codex_agents_dir = codex_home().join("agents");
    let switch_scope = if let Some(name) = args.name.as_deref() {
        let target = write_codex_dynamic_model_for_session(&agents_config_path, name, model)?;
        update_asp_codex_agent_source_and_symlink_projection(
            asp_agents_dir,
            &codex_agents_dir,
            &target,
            model,
            &mut updated_agent_configs,
        )?;
        format!("session:{}", target.session_name)
    } else {
        write_codex_dynamic_model(&agents_config_path, model)?;
        update_asp_codex_agent_sources_and_symlink_projections(
            asp_agents_dir,
            &codex_agents_dir,
            model,
            &mut updated_agent_configs,
        )?;
        "all-codex-asp-agents".to_string()
    };

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "status": "switched",
                "platform": "codex",
                "scope": switch_scope,
                "model": model,
                "configPath": agents_config_path,
                "updatedAgentConfigs": updated_agent_configs,
                "semantics": "configuration-layer",
                "mainSessionModel": "unchanged",
                "childSessionModel": "configured expected model for the selected ASP-managed subagent child session",
                "liveChildSwitch": "send a native message-agent follow-up to the existing child session; this command does not change the main session model or a running child turn",
            })
        );
    } else {
        println!(
            "switched codex child-session config model for {switch_scope} to {model}; main session model unchanged; config={}; updatedAgentConfigs={}; running child sessions still require a native message-agent follow-up",
            agents_config_path.display(),
            updated_agent_configs.len()
        );
    }
    Ok(())
}

fn asp_agents_config_path() -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::state_core::resolve_state_home()?
        .join("agents")
        .join("config.toml"))
}

fn codex_home() -> PathBuf {
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
    let sandbox_verification_status = sandbox_verification_status(
        validation.expected_sandbox.as_deref(),
        validation.actual_sandbox.as_deref(),
    );
    object.insert(
        "validationStatus".to_string(),
        serde_json::Value::String(validation.status.clone()),
    );
    object.insert(
        "validationReason".to_string(),
        serde_json::Value::String(validation.reason.clone()),
    );
    object.insert(
        "validation".to_string(),
        serde_json::json!({
            "status": validation.status,
            "reason": validation.reason,
            "configPath": validation.config_path,
            "rolloutPath": validation.rollout_path,
            "expectedRootSessionId": validation.expected_root_session_id,
            "actualRootSessionId": validation.actual_root_session_id,
            "expectedParentThreadId": validation.expected_parent_thread_id,
            "actualParentThreadId": validation.actual_parent_thread_id,
            "expectedAgentPath": validation.expected_agent_path,
            "actualAgentPath": validation.actual_agent_path,
            "expectedRole": validation.expected_role,
            "actualRole": validation.actual_role,
            "expectedModel": validation.expected_model,
            "actualModel": validation.actual_model,
            "expectedSandbox": validation.expected_sandbox,
            "actualSandbox": validation.actual_sandbox,
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
