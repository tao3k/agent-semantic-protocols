use agent_semantic_client_db::{
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, agent_session_unix_timestamp,
};
use agent_semantic_runtime::agent_session_registration_identity;

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_command_parts::registered_session_is_reusable;
use super::agent_session_registry_render::{
    escape_field, print_json_report, print_reuse_session, print_session_row,
};
use super::agent_session_registry_rollout_activity::rollout_activity_report;
use super::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use super::agent_session_registry_state::{
    current_project_session_scope_id, current_recall_session_id, required_non_empty,
    resolved_root_session_id,
};
use super::agent_session_registry_validation::{
    validate_recent_session_profile, validate_session_profile,
};
use super::normalized_metadata_with_roles;
use std::path::Path;

pub(super) fn register_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let identity = agent_session_registration_identity(
        (
            args.child_session_id.as_deref(),
            args.root_session_id.as_deref(),
        )
            .into(),
    )?;
    let session_id = identity.session_id;
    let now = agent_session_unix_timestamp()?;
    let root_session_id = identity.root_session_id;
    let name = required_non_empty(args.name.as_deref(), "--name")?.to_string();
    let profile =
        super::agent_session_registry_profile::resolve_session_profile_from_args(args, &name)?;
    let roles = profile.roles;
    let role = profile.role;
    let permissions = profile.permissions;
    let status = args.status.as_deref().unwrap_or("active").to_string();
    let validation = if args.replace {
        validate_session_profile(&session_id, &root_session_id, &name, &role, now)?
    } else {
        validate_recent_session_profile(&session_id, &root_session_id, &name, &role, now)?
    };
    let model_observation = validation.actual_model.as_deref().map(|model| {
        agent_semantic_client_db::AgentSessionModelObservationRef {
            model,
            source: agent_semantic_client_db::AgentSessionModelObservationSource::CodexRollout,
            observed_at: now,
            evidence_ref: None,
        }
    });
    let metadata_json = normalized_metadata_with_roles(
        args.metadata_json.as_deref(),
        &validation,
        &roles,
        &permissions,
    )?;
    if validation.status == "failed" {
        let _ = registry.mark_session_invalid(&project_id, &session_id, now);
        let _ = registry.register_session(AgentSessionRegisterRequest {
            project_id: &project_id,
            root_session_id: &root_session_id,
            session_id: &session_id,
            message_target_id: args.message_target_id.as_deref(),
            parent_session_id: args.parent_session_id.as_deref(),
            name: &name,
            role: &role,
            model_observation,
            status: AGENT_SESSION_STATUS_INVALID,
            expires_at: args.expires_at,
            metadata_json: &metadata_json,
            now,
        });
        return Err(format!(
            "agent session validation failed: {}.\nblockedState=validation-failed-or-non-routable-child\nloopCommand=asp agent session bootstrap --name {name}\nagentInstruction=Enter the resident-child choice pane and choose one number. The pane owns status inspection, model alignment, message target recovery, cleanup, creation, and registration; do not run low-level session commands as independent fallback workflows.",
            validation.reason
        ));
    }
    if !args.replace
        && let Some(existing) = registry.lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: None,
            root_session_id: Some(&root_session_id),
            name: Some(&name),
        })?
        && existing.session_id != session_id
        && registered_session_is_reusable(registry, &existing, now)?
    {
        return print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            existing,
            args.json,
        );
    }
    if !args.replace
        && let Some(existing) = adopt_reusable_rollout_session(
            registry,
            RolloutAdoptRequest {
                project_id: &project_id,
                root_session_id: &root_session_id,
                name: &name,
                role: &role,
                roles: &roles,
                permissions: &permissions,
                expires_at: args.expires_at,
                now,
                excluded_session_id: Some(&session_id),
            },
        )?
    {
        return print_reuse_session(
            registry.db_path(),
            Some(&root_session_id),
            existing,
            args.json,
        );
    }

    let record = registry.register_session(AgentSessionRegisterRequest {
        project_id: &project_id,
        root_session_id: &root_session_id,
        session_id: &session_id,
        message_target_id: args.message_target_id.as_deref(),
        parent_session_id: args.parent_session_id.as_deref(),
        name: &name,
        role: &role,
        model_observation,
        status: &status,
        expires_at: args.expires_at,
        metadata_json: &metadata_json,
        now,
    })?;
    if args.json {
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-register] owner=rust rootSession=\"{}\" session=\"{}\" name=\"{}\" role=\"{}\" status=\"{}\" db=\"{}\"",
            escape_field(&root_session_id),
            escape_field(&session_id),
            escape_field(&name),
            escape_field(&role),
            escape_field(&status),
            registry.db_path().display()
        );
        Ok(())
    }
}

pub(super) fn lifecycle_audit_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    super::agent_session_registry_lifecycle_audit::lifecycle_audit_session(registry, args)
}

pub(in crate::command::agent_session_registry) fn stale_invalid_session_should_be_idle(
    record: &AgentSessionRecord,
    now: i64,
) -> Result<bool, String> {
    if record.status != "invalid" {
        return Ok(false);
    }
    let validation = validate_session_profile(
        &record.session_id,
        &record.root_session_id,
        &record.name,
        &record.role,
        now,
    )?;
    if !matches!(validation.status.as_str(), "passed" | "warning" | "skipped") {
        return Ok(false);
    }
    let Some(rollout_path) = validation.rollout_path.as_deref() else {
        return Ok(false);
    };
    let activity = rollout_activity_report(Path::new(rollout_path), now);
    if activity.running_session_closed {
        return Ok(false);
    }
    Ok(activity
        .session_activity
        .as_ref()
        .map(|session_activity| {
            matches!(
                session_activity.status.as_str(),
                "tool-running" | "agent-active" | "idle-resumable"
            )
        })
        .unwrap_or(false))
}

pub(super) fn list_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let root_filter = if args.all {
        None
    } else {
        match args.root_session_id.clone() {
            Some(root_session_id) => Some(root_session_id),
            None => current_recall_session_id(registry)?,
        }
    };
    registry.refresh_expired_sessions()?;
    let mut sessions = registry.query_sessions(
        project_id.as_str(),
        root_filter
            .as_deref()
            .map(agent_semantic_client_db::AgentSessionRootSessionId::from),
        args.name
            .as_deref()
            .map(agent_semantic_client_db::AgentSessionResidentName::from),
    )?;
    if args.active {
        let now = agent_session_unix_timestamp()?;
        sessions.retain(|session| session.is_routable_at(now));
    }
    if args.json {
        return print_json_report(registry.db_path(), root_filter.as_deref(), sessions);
    }
    println!(
        "[agent-session-list] owner=rust rootSession={} sessions={} db=\"{}\"",
        root_filter
            .as_deref()
            .map(|value| format!("\"{}\"", escape_field(value)))
            .unwrap_or_else(|| "\"*\"".to_string()),
        sessions.len(),
        registry.db_path().display()
    );
    for session in sessions {
        print_session_row(&session);
    }
    Ok(())
}

pub(super) fn smoke_session(
    _registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let report = run_invalid_child_bootstrap_smoke()?;
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|error| format!("serialize agent session smoke report: {error}"))?
        );
    } else {
        println!(
            "[agent-session-smoke] scenario=\"{}\" success={} invalidChildBootstrapOk={}",
            report["scenario"]
                .as_str()
                .unwrap_or("invalid-child-bootstrap"),
            report["success"].as_bool().unwrap_or(false),
            report["invalidChildBootstrapOk"].as_bool().unwrap_or(false)
        );
    }
    if report["success"].as_bool().unwrap_or(false) {
        Ok(())
    } else {
        Err("agent session smoke failed".to_string())
    }
}

fn run_invalid_child_bootstrap_smoke() -> Result<serde_json::Value, String> {
    let now = agent_session_unix_timestamp()?;
    let temp_root = std::env::temp_dir().join(format!(
        "asp-agent-session-smoke-{}-{now}",
        std::process::id()
    ));
    let home = temp_root.join("home");
    let codex_home = home.join(".codex");
    let state_home = temp_root.join("asp-state");
    let workspace = temp_root.join("workspace");
    std::fs::create_dir_all(&workspace)
        .map_err(|error| format!("create smoke workspace: {error}"))?;
    let owner_fixture = workspace.join(
        "crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs",
    );
    if let Some(parent) = owner_fixture.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create smoke owner fixture parent: {error}"))?;
    }
    std::fs::write(&owner_fixture, "pub fn message_target_snapshot() {}\n")
        .map_err(|error| format!("write smoke owner fixture: {error}"))?;
    let root_session_id = "asp-smoke-root-session";
    let child_session_id = "asp-smoke-invalid-child";
    write_smoke_codex_agent_fixture(&codex_home)?;
    write_smoke_codex_rollout_fixture(&codex_home, &workspace, root_session_id, child_session_id)?;
    let asp_bin = std::env::current_exe()
        .map_err(|error| format!("resolve current asp executable for smoke: {error}"))?;
    let register = std::process::Command::new(&asp_bin)
        .current_dir(&workspace)
        .env("HOME", &home)
        .env("CODEX_HOME", &codex_home)
        .env("ASP_STATE_HOME", &state_home)
        .args([
            "agent",
            "session",
            "register",
            "--name",
            "asp-explore",
            "--child-session-id",
            child_session_id,
            "--root-session-id",
            root_session_id,
            "--roles",
            "subagent,search",
            "--status",
            "invalid",
        ])
        .output()
        .map_err(|error| format!("run smoke register: {error}"))?;
    let register_output = command_output_text(&register);
    let denied = if register.status.success() {
        Some(
            std::process::Command::new(&asp_bin)
                .current_dir(&workspace)
                .env("HOME", &home)
                .env("CODEX_HOME", &codex_home)
                .env("ASP_STATE_HOME", &state_home)
                .env("CODEX_THREAD_ID", root_session_id)
                .args([
                    "rust",
                    "search",
                    "owner",
                    "crates/agent-semantic-protocol/src/command/agent_session_registry_message_target.rs",
                    "items",
                    "--query",
                    "message_target_snapshot",
                    "--workspace",
                    ".",
                    "--view",
                    "seeds",
                ])
                .output()
                .map_err(|error| format!("run smoke denied search: {error}"))?,
        )
    } else {
        None
    };
    let denied_output = denied.as_ref().map(command_output_text).unwrap_or_default();
    let invalid_child_bootstrap_ok = register.status.success()
        && denied
            .as_ref()
            .is_some_and(|output| output.status.success())
        && denied_output.contains("[search-owner]")
        && !denied_output.contains("reuse");
    let report = serde_json::json!({
        "action": "agent-session-smoke",
        "scenario": "invalid-child-bootstrap",
        "success": invalid_child_bootstrap_ok,
        "registerOk": register.status.success(),
        "deniedSearchRejected": denied.as_ref().is_some_and(|output| !output.status.success()),
        "invalidChildBootstrapOk": invalid_child_bootstrap_ok,
        "tempStateRoot": state_home.display().to_string(),
        "blockers": if invalid_child_bootstrap_ok {
            Vec::<String>::new()
        } else {
            vec![format!(
                "registerOutput={} deniedOutput={}",
                compact_smoke_output(&register_output),
                compact_smoke_output(&denied_output)
            )]
        },
    });
    let _ = std::fs::remove_dir_all(&temp_root);
    Ok(report)
}

fn write_smoke_codex_agent_fixture(codex_home: &std::path::Path) -> Result<(), String> {
    let agents_dir = codex_home.join("agents");
    std::fs::create_dir_all(&agents_dir)
        .map_err(|error| format!("create smoke codex agents dir: {error}"))?;
    std::fs::write(
        agents_dir.join("asp-explorer.toml"),
        r#"name = "asp_explorer"
description = "ASP reasoning and evidence exploration lane."
nickname_candidates = ["ASP Explore", "ASP Reasoning"]
model = "gpt-5.4-mini"
model_reasoning_effort = "low"
sandbox_mode = "read-only"
developer_instructions = "ASP smoke fixture."
"#,
    )
    .map_err(|error| format!("write smoke codex agent fixture: {error}"))?;
    Ok(())
}

fn write_smoke_codex_rollout_fixture(
    codex_home: &std::path::Path,
    workspace: &std::path::Path,
    root_session_id: &str,
    child_session_id: &str,
) -> Result<(), String> {
    let rollout_dir = codex_home.join("sessions/2026/07/06");
    std::fs::create_dir_all(&rollout_dir)
        .map_err(|error| format!("create smoke rollout dir: {error}"))?;
    let root_rollout_path = rollout_dir.join(format!(
        "rollout-2026-07-06T00-00-00-{root_session_id}.jsonl"
    ));
    let child_rollout_path = rollout_dir.join(format!(
        "rollout-2026-07-06T00-00-00-{child_session_id}.jsonl"
    ));
    let workspace_json = serde_json::to_string(&workspace.display().to_string())
        .map_err(|error| format!("serialize smoke workspace: {error}"))?;
    let root_session_meta = serde_json::json!({
        "timestamp": "2026-07-06T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "id": root_session_id,
            "session_id": root_session_id,
            "timestamp": "2026-07-06T00:00:00.000Z",
            "cwd": workspace.display().to_string(),
            "originator": "Codex Desktop",
            "cli_version": "0.142.5",
            "thread_source": "local",
            "model_provider": "openai"
        }
    });
    let child_session_meta = serde_json::json!({
        "timestamp": "2026-07-06T00:00:00.000Z",
        "type": "session_meta",
        "payload": {
            "session_id": root_session_id,
            "id": child_session_id,
            "parent_thread_id": root_session_id,
            "timestamp": "2026-07-06T00:00:00.000Z",
            "cwd": workspace.display().to_string(),
            "originator": "Codex Desktop",
            "cli_version": "0.142.5",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": root_session_id,
                        "depth": 1,
                        "agent_path": null,
                        "agent_nickname": "ASP Explore",
                        "agent_role": "asp_explorer"
                    }
                }
            },
            "thread_source": "subagent",
            "agent_nickname": "ASP Explore",
            "agent_role": "asp_explorer",
            "model_provider": "openai"
        }
    });
    let turn_context = format!(
        "{{\"timestamp\":\"2026-07-06T00:00:00.001Z\",\"type\":\"turn_context\",\"payload\":{{\"turn_id\":\"asp-smoke-turn\",\"cwd\":{workspace_json},\"workspace_roots\":[{workspace_json}],\"approval_policy\":\"never\",\"sandbox_policy\":{{\"type\":\"read-only\"}},\"model\":\"gpt-5.4-mini\"}}}}"
    );
    let root_content = format!("{root_session_meta}\n{turn_context}\n");
    let child_content = format!("{child_session_meta}\n{turn_context}\n");
    std::fs::write(&root_rollout_path, root_content)
        .map_err(|error| format!("write smoke root rollout fixture: {error}"))?;
    std::fs::write(&child_rollout_path, child_content)
        .map_err(|error| format!("write smoke child rollout fixture: {error}"))?;
    Ok(())
}

fn command_output_text(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn compact_smoke_output(value: &str) -> String {
    value
        .split_whitespace()
        .take(80)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn show_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    let name = if args.child_session_id.is_some() {
        None
    } else {
        Some(required_non_empty(
            args.name.as_deref(),
            "--name or --child-session-id",
        )?)
    };
    let root_session_id = if args.child_session_id.is_some() {
        None
    } else {
        Some(
            resolved_root_session_id(registry, args.root_session_id.as_deref())?.ok_or_else(
                || {
                    "asp agent session show --name requires --root-session-id or agent session env"
                        .to_string()
                },
            )?,
        )
    };
    let record = registry
        .lookup_session(AgentSessionLookupRequest {
            project_id: &project_id,
            session_id: args.child_session_id.as_deref(),
            root_session_id: root_session_id.as_deref(),
            name,
        })?
        .ok_or_else(|| "session registry entry not found".to_string())?;

    if args.json {
        let root_session_id = record.root_session_id.clone();
        print_json_report(registry.db_path(), Some(&root_session_id), vec![record])
    } else {
        println!(
            "[agent-session-show] owner=rust rootSession=\"{}\" sessions=1 db=\"{}\"",
            escape_field(&record.root_session_id),
            registry.db_path().display()
        );
        print_session_row(&record);
        Ok(())
    }
}
