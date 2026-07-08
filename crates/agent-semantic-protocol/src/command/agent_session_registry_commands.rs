use agent_semantic_client_db::{
    AGENT_SESSION_STATUS_INVALID, AgentSessionLookupRequest, AgentSessionRecord,
    AgentSessionRegisterRequest, AgentSessionRegistry, agent_session_unix_timestamp,
};
use agent_semantic_runtime::{
    agent_session_registration_identity, agent_session_runtime_status_snapshot,
};

use super::agent_session_registry_args::SessionArgs;
use super::agent_session_registry_lifetime::resolve_session_lifetime;
use super::agent_session_registry_render::{
    SessionStatusReport, escape_field, print_json_report, print_session_row, print_status_report,
};
use super::agent_session_registry_rollout_activity::rollout_activity_report;
use super::agent_session_registry_rollout_adopt::{
    RolloutAdoptRequest, adopt_reusable_rollout_session,
};
use super::agent_session_registry_state::{
    current_project_session_scope_id, current_recall_session_id, project_session_scope_id,
    required_non_empty, resolved_root_session_id, session_record_validation_allows_routing,
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
            model: args.model.as_deref(),
            status: AGENT_SESSION_STATUS_INVALID,
            expires_at: args.expires_at,
            metadata_json: &metadata_json,
            now,
        });
        return Err(format!(
            "agent session validation failed: {}.\nblockedState=validation-failed-or-non-routable-child\nnextAction=ask-existing-child-to-switch-model-and-revalidate\nstatusCommand=asp agent session status --name {name} --json\nagentInstruction=Use the existing `{name}` child session. If this is a model mismatch, switch that same Codex child to the requiredModel shown in the validation reason, then rerun status. Do not create or replace the child.\nconfigSwitchCommandTemplate=asp agent session switch-model --name {name} --model <requiredModel> --json\nconfigSwitchPurpose=Use the config switch command only when the underlying ASP/Codex model mapping or capacity fallback configuration must be updated.",
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
                model: args.model.as_deref(),
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
        model: args.model.as_deref(),
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

fn stale_invalid_session_should_be_idle(
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
    let mut sessions =
        registry.query_sessions(&project_id, root_filter.as_deref(), args.name.as_deref())?;
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
            .is_some_and(|output| !output.status.success())
        && denied_output.contains("childStatus=invalid")
        && denied_output.contains("do not use the existing asp-explore child")
        && denied_output.contains("cleanup")
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
    registry.refresh_expired_sessions()?;
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

pub(super) fn status_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_root: &Path,
) -> Result<(), String> {
    if args.activity
        && let Some(child_session_id) = args.child_session_id.as_deref()
    {
        let now = agent_session_unix_timestamp()?;
        if let Some(rollout_path) =
            super::agent_session_registry_rollout_lookup::fast_rollout_path_for_session_id(
                child_session_id,
            )
        {
            let report = rollout_activity_report(&rollout_path, now);
            return super::agent_session_registry_render::print_status_activity_report(
                Some(&report),
                report.agent_instruction.as_str(),
                args.json,
            );
        }
    }

    let project_id = project_session_scope_id(registry, project_root)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let name = args.name.clone();
    let mut record = registry.lookup_session(AgentSessionLookupRequest {
        project_id: &project_id,
        session_id: args.child_session_id.as_deref(),
        root_session_id: root_session_id.as_deref(),
        name: name.as_deref(),
    })?;
    let now = agent_session_unix_timestamp()?;
    if let Some(session) = record.as_mut()
        && stale_invalid_session_should_be_idle(session, now)?
    {
        registry.update_session_status(&project_id, &session.session_id, "idle", now)?;
        session.status = "idle".to_string();
        session.updated_at = now;
        session.last_seen_at = Some(now);
    }
    let validation = record
        .as_ref()
        .map(|session| {
            validate_session_profile(
                &session.session_id,
                &session.root_session_id,
                &session.name,
                &session.role,
                now,
            )
        })
        .transpose()?;
    if let (Some(session), Some(validation)) = (record.as_ref(), validation.as_ref())
        && validation.status == "failed"
    {
        let _ = registry.mark_session_invalid(&project_id, &session.session_id, now);
    }
    let registry_allows_routing = record
        .as_ref()
        .map(|session| {
            !matches!(session.status.as_str(), "archived" | "closed") && session.is_routable_at(now)
        })
        .unwrap_or(false);
    let validation_allows_routing = validation.as_ref().is_none_or(|validation| {
        matches!(validation.status.as_str(), "passed" | "warning" | "skipped")
    });
    let validation_next_action = validation.as_ref().and_then(session_validation_next_action);
    let required_model = validation
        .as_ref()
        .and_then(|validation| validation.expected_model.clone());
    let actual_model = validation
        .as_ref()
        .and_then(|validation| validation.actual_model.clone());
    let rollout_activity = validation
        .as_ref()
        .and_then(|validation| validation.rollout_path.as_deref())
        .map(|rollout_path| rollout_activity_report(Path::new(rollout_path), now));
    if args.activity {
        let next_action = rollout_activity
            .as_ref()
            .map(|activity| activity.agent_instruction.as_str())
            .unwrap_or("register-existing-child-or-replace-only-after-host-confirms-unrecoverable");
        return super::agent_session_registry_render::print_status_activity_report(
            rollout_activity.as_ref(),
            next_action,
            args.json,
        );
    }
    let lifecycle_allows_routing = rollout_activity
        .as_ref()
        .map(|activity| !activity.running_session_closed)
        .unwrap_or(true);
    let mut routable =
        registry_allows_routing && validation_allows_routing && lifecycle_allows_routing;
    let registry_status = record
        .as_ref()
        .map(|session| session.status.clone())
        .unwrap_or_else(|| "missing".to_string());
    let host_thread_id = record
        .as_ref()
        .map(|session| session.session_id.as_str())
        .or(root_session_id.as_deref());
    let runtime_status = agent_session_runtime_status_snapshot(
        (
            project_root,
            now,
            args.artifact_stale_after_seconds,
            host_thread_id,
            record.is_some(),
            registry_allows_routing,
        )
            .into(),
    )?;
    let rollout_session_index = None;
    if routable
        && rollout_activity
            .as_ref()
            .and_then(|activity| activity.session_activity.as_ref())
            .is_some_and(|activity| {
                matches!(
                    activity.status.as_str(),
                    "tool-running" | "agent-active" | "idle-resumable"
                )
            })
    {
        routable = true;
    }
    let session_lifecycle_index = None;
    let activity_snapshot_short = None;
    let (host_thread_existence, host_thread_existence_reason) =
        host_thread_existence_snapshot(runtime_status.host_thread_id.as_deref());
    let multi_agent_child_state = multi_agent_child_state_snapshot(rollout_activity.as_ref());
    let session_lifetime = resolve_session_lifetime(
        project_root,
        name.as_deref(),
        runtime_status.host_client.as_deref(),
    );
    let mut report = SessionStatusReport {
        owner: "rust",
        db_path: registry.db_path().display().to_string(),
        root_session_id,
        name,
        session: record,
        registry_status,
        routable,
        session_lifetime: session_lifetime.value,
        resident: session_lifetime.resident,
        session_lifetime_source: session_lifetime.source,
        validation_status: validation
            .as_ref()
            .map(|validation| validation.status.clone())
            .unwrap_or_else(|| "missing-registry".to_string()),
        validation_reason: validation
            .as_ref()
            .map(|validation| validation.reason.clone())
            .unwrap_or_else(|| "session registry entry not found".to_string()),
        validation,
        rollout_session_index,
        rollout_activity,
        session_lifecycle_index,
        activity_snapshot_short,
        host_client: runtime_status.host_client,
        host_thread_id: runtime_status.host_thread_id,
        host_status_source: runtime_status.host_status_source,
        host_status: runtime_status.host_status,
        host_status_reason: runtime_status.host_status_reason,
        host_thread_existence,
        host_thread_existence_reason,
        multi_agent_child_state,
        message_target_status: None,
        message_target_result_source: None,
        message_agent_target_id: None,
        message_agent_target_id_equals_child: None,
        host_raw_status: runtime_status.host_raw_status,
        health_status: runtime_status.health_status,
        timeout_semantics: runtime_status.timeout_semantics,
        duplicate_worker_allowed: runtime_status.duplicate_worker_allowed,
        artifacts_dir: runtime_status.artifacts_dir,
        artifact_status: runtime_status.artifact_status,
        artifact_stale_after_seconds: runtime_status.artifact_stale_after_seconds,
        last_artifact_updated_at: runtime_status.last_artifact_updated_at,
        artifact_age_seconds: runtime_status.artifact_age_seconds,
        last_artifact_path: runtime_status.last_artifact_path,
        next_action: runtime_status.next_action,
        required_model,
        actual_model,
        model_alignment_action: None,
        model_alignment_message: None,
    };
    if report.routable && report.required_model.is_some() {
        report.model_alignment_action = Some(
            "parent-send-codex-follow-up-with-required-model-override-and-revalidate".to_string(),
        );
        let required_model = report.required_model.as_deref().unwrap_or("");
        let name = report.name.as_deref().unwrap_or("asp-explore");
        report.model_alignment_message = Some(format!(
            "After native resume or before the next child task, the parent must send a Codex follow-up to this same child thread with model override {required_model} and light/low reasoning, wait for the child receipt, then rerun asp agent session status --name {name}."
        ));
    }
    if let Some(activity) = report.rollout_activity.as_ref() {
        report.next_action = activity.agent_instruction.clone();
    }
    if let Some(next_action) = validation_next_action {
        report.next_action = next_action.to_string();
    }
    if report.routable && report.health_status == "registry-not-routable" {
        report.health_status = "unknown".to_string();
        if report.rollout_activity.is_none() {
            report.next_action =
                "resume-or-send-follow-up-to-same-child-before-considering-replacement".to_string();
        }
    }
    print_status_report(report, args.json)
}

fn session_validation_next_action(
    validation: &agent_semantic_runtime::AgentSessionValidationReport,
) -> Option<&'static str> {
    if validation.status == "warning"
        && validation
            .reason
            .contains("requiredAction=parent-send-message-same-child-with-required-model")
    {
        return Some("resume-or-send-follow-up-to-same-child-before-considering-replacement");
    }
    None
}

fn registered_session_is_reusable(
    registry: &AgentSessionRegistry,
    record: &AgentSessionRecord,
    now: i64,
) -> Result<bool, String> {
    if matches!(record.status.as_str(), "archived" | "closed") {
        return Ok(false);
    }
    if !record.is_routable_at(now) {
        return Ok(false);
    }
    if !session_record_validation_allows_routing(registry, record, now)? {
        return Ok(false);
    }
    let validation = validate_session_profile(
        &record.session_id,
        &record.root_session_id,
        &record.name,
        &record.role,
        now,
    )?;
    Ok(validation
        .rollout_path
        .as_deref()
        .map(|rollout_path| {
            let activity = rollout_activity_report(Path::new(rollout_path), now);
            activity
                .session_activity
                .as_ref()
                .map(|session_activity| {
                    matches!(
                        session_activity.status.as_str(),
                        "tool-running" | "agent-active" | "idle-resumable"
                    )
                })
                .unwrap_or(!activity.running_session_closed)
        })
        .unwrap_or(true))
}

fn host_thread_existence_snapshot(host_thread_id: Option<&str>) -> (String, String) {
    let Some(host_thread_id) = host_thread_id else {
        return (
            "not-applicable".to_string(),
            "no hostThreadId is available for this status request".to_string(),
        );
    };
    let current_thread_id = std::env::var("CODEX_THREAD_ID").ok();
    if current_thread_id.as_deref() == Some(host_thread_id) {
        return (
            "current-thread-active".to_string(),
            "hostThreadId matches the current CODEX_THREAD_ID; this proves the root Codex thread is present".to_string(),
        );
    }
    (
        "not-validated".to_string(),
        "ASP lifecycle does not use non-structural Codex thread listing as a control-plane source; host presence is not proven unless current CODEX_THREAD_ID, rollout ledger, and registry identity agree".to_string(),
    )
}

fn multi_agent_child_state_snapshot(
    rollout_activity: Option<
        &super::agent_session_registry_rollout_activity::RolloutActivityReport,
    >,
) -> String {
    match rollout_activity {
        Some(activity)
            if activity
                .session_activity
                .as_ref()
                .is_some_and(|session_activity| session_activity.status == "idle-resumable") =>
        {
            "control-plane-idle-resumable".to_string()
        }
        Some(activity)
            if activity
                .session_activity
                .as_ref()
                .is_some_and(|session_activity| {
                    matches!(
                        session_activity.status.as_str(),
                        "tool-running" | "agent-active"
                    )
                }) =>
        {
            "control-plane-running-session-open-or-unknown".to_string()
        }
        Some(activity) if activity.running_session_closed => {
            "control-plane-running-session-closed".to_string()
        }
        Some(_) => "control-plane-running-session-open-or-unknown".to_string(),
        None => "not-reported".to_string(),
    }
}

pub(super) fn close_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    registry.refresh_expired_sessions()?;
    let record = lifecycle_target_session(registry, args, &project_id)?;
    let now = agent_session_unix_timestamp()?;
    let archived = registry.archive_session(&project_id, &record.session_id, now)?;
    if args.json {
        print_lifecycle_json(
            "close",
            std::slice::from_ref(&record.session_id),
            1,
            usize::from(archived),
            Some("archived"),
        )
    } else {
        println!(
            "[agent-session-close] archived={} sessionId={} name={} role={}",
            archived, record.session_id, record.name, record.role
        );
        Ok(())
    }
}

pub(super) fn gc_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    registry.refresh_expired_sessions()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let candidates = if args.child_session_id.is_some() {
        lifecycle_target_session(registry, args, &project_id).map(|record| vec![record])?
    } else {
        registry.query_sessions(
            &project_id,
            root_session_id.as_deref(),
            args.name.as_deref(),
        )?
    };
    let mut inspected = 0usize;
    let mut deleted = Vec::new();
    for record in candidates {
        inspected += 1;
        if (args.force || is_gc_candidate_status(&record.status))
            && registry.delete_session(&project_id, &record.session_id)?
        {
            deleted.push(record.session_id);
        }
    }
    if args.json {
        print_lifecycle_json("gc", &deleted, inspected, deleted.len(), None)
    } else {
        println!(
            "[agent-session-gc] inspected={} deleted={}",
            inspected,
            deleted.len()
        );
        for session_id in deleted {
            println!("{session_id}");
        }
        Ok(())
    }
}

pub(super) fn reconcile_sessions(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
) -> Result<(), String> {
    let project_id = current_project_session_scope_id(registry)?;
    registry.refresh_expired_sessions()?;
    let now = agent_session_unix_timestamp()?;
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    let sessions = registry.query_sessions(
        &project_id,
        root_session_id.as_deref(),
        args.name.as_deref(),
    )?;
    let mut reconciled_session_ids = Vec::new();
    for record in &sessions {
        if stale_invalid_session_should_be_idle(record, now)?
            && registry.update_session_status(&project_id, &record.session_id, "idle", now)?
        {
            reconciled_session_ids.push(record.session_id.clone());
        }
    }
    let gc_candidates = sessions
        .iter()
        .filter(|record| is_gc_candidate_status(&record.status))
        .count();
    if args.json {
        print_lifecycle_json(
            "reconcile",
            &reconciled_session_ids,
            sessions.len(),
            gc_candidates,
            Some("refreshed-expired-and-reconciled-rollout-sessions"),
        )
    } else {
        println!(
            "[agent-session-reconcile] refreshed=true reconciled={} sessions={} gcCandidates={}",
            reconciled_session_ids.len(),
            sessions.len(),
            gc_candidates
        );
        Ok(())
    }
}

fn lifecycle_target_session(
    registry: &AgentSessionRegistry,
    args: &SessionArgs,
    project_id: &str,
) -> Result<AgentSessionRecord, String> {
    if args.child_session_id.is_none() && args.name.is_none() {
        return Err("session lifecycle command requires --child-session-id or --name".to_string());
    }
    let root_session_id = resolved_root_session_id(registry, args.root_session_id.as_deref())?;
    registry
        .lookup_session(AgentSessionLookupRequest {
            project_id,
            session_id: args.child_session_id.as_deref(),
            root_session_id: root_session_id.as_deref(),
            name: args.name.as_deref(),
        })?
        .ok_or_else(|| "session lifecycle target not found".to_string())
}

fn is_gc_candidate_status(status: &str) -> bool {
    matches!(
        status,
        "archived" | "closed" | "expired" | AGENT_SESSION_STATUS_INVALID
    )
}

fn print_lifecycle_json(
    command: &str,
    session_ids: &[String],
    inspected: usize,
    affected: usize,
    status: Option<&str>,
) -> Result<(), String> {
    let report = serde_json::json!({
        "owner": "rust",
        "command": command,
        "inspected": inspected,
        "affected": affected,
        "status": status,
        "sessionIds": session_ids,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to render lifecycle json: {error}"))?
    );
    Ok(())
}
use super::agent_session_registry_render::print_reuse_session;
