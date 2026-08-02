//! Runtime for the `asp hook` command surface.

#[path = "hook_runtime_activation_failure.rs"]
mod hook_runtime_activation_failure;
#[path = "hook_runtime_activation_path.rs"]
mod hook_runtime_activation_path;
#[path = "hook_runtime_agent_session.rs"]
mod hook_runtime_agent_session;
#[path = "hook_runtime_agent_session_dispatch.rs"]
mod hook_runtime_agent_session_dispatch;
#[path = "hook_runtime_bootstrap.rs"]
mod hook_runtime_bootstrap;
#[path = "hook_runtime_cli_args.rs"]
mod hook_runtime_cli_args;
#[path = "hook_runtime_codex_plugin.rs"]
mod hook_runtime_codex_plugin;
#[path = "hook_runtime_codex_plugin_identity.rs"]
mod hook_runtime_codex_plugin_identity;
#[path = "hook_runtime_config_recovery.rs"]
mod hook_runtime_config_recovery;
#[path = "hook_runtime_decision_render.rs"]
mod hook_runtime_decision_render;
#[path = "hook_runtime_doctor.rs"]
mod hook_runtime_doctor;
#[path = "hook_runtime_generation_admission.rs"]
mod hook_runtime_generation_admission;
pub(super) use hook_runtime_generation_admission::request as request_runtime_generation_admission;
#[path = "hook_runtime_install.rs"]
mod hook_runtime_install;
#[path = "hook_runtime_project_root_override.rs"]
mod hook_runtime_project_root_override;
#[path = "hook_runtime_resident_permissions.rs"]
mod hook_runtime_resident_permissions;
#[path = "hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "hook_runtime_source_access_materialize.rs"]
mod hook_runtime_source_access_materialize;
#[path = "hook_runtime_stdin.rs"]
pub(crate) mod hook_runtime_stdin;
#[path = "hook_runtime_subagent.rs"]
mod hook_runtime_subagent;
#[path = "hook_runtime_workspace_candidate.rs"]
mod hook_runtime_workspace_candidate;

pub(super) use hook_runtime_skill::active_codex_plugin_skill_path;

#[cfg(test)]
#[path = "../../tests/unit/command/hook_workspace_candidate.rs"]
mod hook_workspace_candidate_tests;

use super::{codex_enforcement_report, payload_indicates_subagent_context};
use agent_semantic_client_db::{AgentSessionLookupRequest, AgentSessionRegistry};
use agent_semantic_hook::{
    ActiveContextRecord, DecisionKind, HookClassificationRequest, HookDecision, ReasonKind,
    append_hook_event_state, apply_repeated_deny_replay, classify_hook_with_config,
    default_client_config_path, has_recorded_subagent_context, load_activation, parse_payload,
    record_active_context, subagent_deny_message,
};
use agent_semantic_runtime::project_state_paths;
use hook_runtime_activation_failure::emit_activation_load_failure;
use hook_runtime_activation_path::default_or_discovered_activation_path;
use hook_runtime_agent_session::{classify_main_session_asp_exploration, load_asp_session_policy};
use hook_runtime_cli_args::{display_path, optional_flag_value};
use hook_runtime_codex_plugin::codex_project_plugin_hooks_present;
use hook_runtime_config_recovery::annotate_hook_config_repair;
use hook_runtime_decision_render::{emit_decision, emit_hook_runtime_failure};
use hook_runtime_doctor::run_doctor;
pub(super) use hook_runtime_install::run_codex_plugin_install_args;
use hook_runtime_install::run_install;
use hook_runtime_project_root_override::{
    hook_runtime_project_root_override, with_hook_runtime_project_root_override,
};
use hook_runtime_resident_permissions::{
    classify_read_only_resident_receipt, classify_read_only_resident_write,
};
use hook_runtime_source_access_materialize::materialize_source_access_deny_message;
use hook_runtime_stdin::read_hook_stdin_bounded;
use hook_runtime_workspace_candidate::{hook_workspace_candidate, requests_explicit_asp_workspace};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn run_hook_runtime_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run(args.into_iter().map(Into::into).collect())
}

pub(crate) fn run_protocol_hook_with_input(args: Vec<String>, input: String) -> Result<(), String> {
    hook_runtime_stdin::with_hook_stdin_override(input, || super::run_protocol_command(args))
}

pub(crate) fn run_protocol_hook_capture(
    args: Vec<String>,
    input: String,
) -> Result<String, String> {
    let (_, output) = hook_runtime_decision_render::with_hook_output_capture(|| {
        run_protocol_hook_with_input(args, input)
    })?;
    if output.is_empty() {
        return Err("hook evaluation completed without a platform response".to_owned());
    }
    Ok(output)
}

pub(crate) fn run_protocol_hook_capture_for_project(
    project_root: &Path,
    args: Vec<String>,
    input: String,
) -> Result<String, String> {
    let project_root = fs::canonicalize(project_root).map_err(|error| {
        format!(
            "failed to canonicalize resident hook workspace {}: {error}",
            project_root.display()
        )
    })?;
    with_hook_runtime_project_root_override(project_root, || run_protocol_hook_capture(args, input))
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("hook") => run_hook(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]),
        Some("install") => run_install(&args[1..]),
        Some("paths") => run_paths(&args[1..]),
        _ => Err(
            "usage: asp hook <install|doctor|paths|hook> --client codex [PROJECT_ROOT]".to_string(),
        ),
    }
}

fn run_paths(args: &[String]) -> Result<(), String> {
    let project_root = project_root_arg(args)?;
    let paths = project_state_paths(&project_root)?;
    println!("projectRoot={}", project_root.display());
    println!("protocolHome={}", paths.protocol_home.display());
    println!("hookCacheDir={}", paths.hook_cache_dir.display());
    println!("hookStateDir={}", paths.hook_state_dir.display());
    println!("activation={}", paths.activation_path.display());
    println!("clientCacheDir={}", paths.client_cache_dir.display());
    println!("artifactsDir={}", paths.artifacts_dir.display());
    println!("runtimeHome={}", paths.runtime_home.display());
    println!("runtimeBinDir={}", paths.runtime_bin_dir.display());
    println!("providerLockDir={}", paths.provider_lock_dir.display());
    Ok(())
}

fn run_hook(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    ensure_supported_client(client)?;
    let emit = flag_value(args, "--emit").unwrap_or("platform");
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    let classification_event = if client == "codex" && event == "permission-request" {
        "pre-tool"
    } else {
        event
    };
    let explicit_activation_path = flag_value(args, "--activation").map(PathBuf::from);
    let stdin = match read_hook_stdin_bounded() {
        Ok(stdin) => stdin,
        Err(error) => {
            emit_hook_runtime_failure(
                client,
                event,
                emit,
                &format!("failed to read hook payload from stdin: {error}"),
            )?;
            return Ok(());
        }
    };
    let payload = match parse_payload(&stdin) {
        Ok(payload) => payload,
        Err(error) => {
            emit_hook_runtime_failure(
                client,
                event,
                emit,
                &format!("invalid hook payload JSON: {error:?}"),
            )?;
            return Ok(());
        }
    };
    if hook_runtime_bootstrap::emit_asp_no_agent_receipt_if_requested(
        client, event, emit, &payload,
    )? {
        return Ok(());
    }
    let activation_path =
        explicit_activation_path.unwrap_or_else(|| default_or_discovered_activation_path(&payload));
    let mut activation_auto_refresh = None;
    let mut runtime = match load_activation(&activation_path) {
        Ok(registry) => registry,
        Err(initial_error) => {
            let repair_project_root = match activation_repair_project_root(
                &payload,
                &activation_path,
            ) {
                Ok(project_root) => project_root,
                Err(identity_error) => {
                    emit_activation_load_failure(
                        client,
                        event,
                        emit,
                        &activation_path,
                        &format!(
                            "initial load failed: {initial_error}; activation repair identity resolution failed: {identity_error}"
                        ),
                        &stdin,
                    )?;
                    return Ok(());
                }
            };
            match agent_semantic_hook::load_or_sync_activation(
                &activation_path,
                &repair_project_root,
            ) {
                Ok(registry) => {
                    activation_auto_refresh = Some("completed:activation-refresh".to_string());
                    registry
                }
                Err(reload_error) => {
                    emit_activation_load_failure(
                        client,
                        event,
                        emit,
                        &activation_path,
                        &format!(
                            "initial load failed: {initial_error}; automatic activation refresh failed: {reload_error}"
                        ),
                        &stdin,
                    )?;
                    return Ok(());
                }
            }
        }
    };
    let activation_project_root =
        hook_runtime_project_root(&activation_path, &runtime.project_root);
    let project_root = hook_workspace_candidate(&payload, &activation_project_root);
    runtime.project_root = project_root.display().to_string();
    let config_path = flag_value(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_client_config_path(&project_root.to_string_lossy()));
    let (loaded_hook_config, hook_matcher_generation_status) =
        hook_runtime_config_recovery::load_fresh_hook_config(
            &config_path,
            &project_root,
            &runtime,
        )?;
    let hook_config = &loaded_hook_config.config;
    let asp_session_policy = &loaded_hook_config.asp_session_policy;
    let hook_config_repair_reasons = &loaded_hook_config.repair_reasons;
    let hook_config_auto_refresh = loaded_hook_config.auto_refresh.as_deref();
    let hook_config_refresh_receipt = hook_config_auto_refresh.unwrap_or("not-required");
    hook_runtime_config_recovery::apply_language_provider_projection(
        hook_config,
        &mut runtime,
        &config_path,
        hook_config_refresh_receipt,
    )?;
    let agent_session_decision = if classification_event == "pre-tool" {
        None
    } else {
        classify_main_session_asp_exploration(
            &project_root,
            client,
            classification_event,
            asp_session_policy,
            &payload,
        )?
    };
    let classified_decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: hook_config,
        platform: client,
        event: classification_event,
        payload: &payload,
    });
    let workspace_mutated =
        hook_runtime_generation_admission::decision_mutates_workspace(&classified_decision);
    let changed_paths =
        hook_runtime_generation_admission::decision_changed_paths(&classified_decision);
    let mutation_id = hook_runtime_generation_admission::decision_mutation_id(&classified_decision);
    let mut decision = if let Some(read_only_decision) = classify_read_only_resident_receipt(
        &project_root,
        client,
        classification_event,
        asp_session_policy,
        &payload,
    ) {
        read_only_decision
    } else if let Some(read_only_decision) = classify_read_only_resident_write(
        &project_root,
        client,
        classification_event,
        asp_session_policy,
        &payload,
    ) {
        read_only_decision
    } else if let Some(agent_session_decision) = agent_session_decision {
        agent_session_decision
    } else {
        classified_decision
    };
    decision.fields.insert(
        "hookMatcherGeneration".to_owned(),
        serde_json::Value::String(hook_matcher_generation_status.to_owned()),
    );
    decision.event = event.to_string();
    hook_runtime_agent_session_dispatch::enforce_configured_resident_spawn_contract(
        hook_config,
        client,
        classification_event,
        &payload,
        &mut decision,
    );
    hook_runtime_agent_session_dispatch::materialize_resident_dispatch_wrapper(
        &payload,
        &mut decision,
    );
    let runtime_generation_admission = hook_runtime_generation_admission::observe(
        args,
        workspace_mutated,
        mutation_id,
        changed_paths,
        requests_explicit_asp_workspace(&payload),
        |mutation_id, changed_paths| {
            hook_runtime_generation_admission::request(&project_root, mutation_id, changed_paths)
        },
        || hook_runtime_generation_admission::ensure(&project_root),
    );
    if let Some(auto_refresh) = hook_config_auto_refresh {
        annotate_hook_config_repair(
            &mut decision,
            &config_path,
            hook_config_repair_reasons.as_slice(),
            auto_refresh,
        );
    }
    if let Some(receipt) = activation_auto_refresh {
        decision.fields.insert(
            "activationAutoRefresh".to_string(),
            serde_json::Value::String(receipt),
        );
        decision.fields.insert(
            "activationRecoveryStatus".to_string(),
            serde_json::Value::String("reloaded-and-classified".to_string()),
        );
    }
    if let Some(receipt) = runtime_generation_admission.receipt {
        decision
            .fields
            .insert("runtimeGenerationAdmission".to_owned(), receipt);
    }
    if let Some(error) = runtime_generation_admission.error {
        decision.fields.insert(
            "runtimeGenerationAdmissionStatus".to_owned(),
            serde_json::Value::String("failed-non-blocking".to_owned()),
        );
        decision.fields.insert(
            "runtimeGenerationAdmissionError".to_owned(),
            serde_json::Value::String(error),
        );
    }
    if matches!(event, "subagent-start" | "subagent-stop") {
        let mut payload_keys = payload
            .as_object()
            .map(|object| object.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        payload_keys.sort();
        decision.fields.insert(
            "hookPayloadKeys".to_string(),
            serde_json::json!(payload_keys),
        );
        for (payload_key, field_key) in [
            ("agent_id", "hookObservedChildId"),
            ("session_id", "hookObservedRootSessionId"),
            ("agent_type", "hookObservedAgentType"),
            ("model", "hookObservedModel"),
            ("reasoning_effort", "hookObservedReasoningEffort"),
            ("permission_mode", "hookObservedPermissionMode"),
        ] {
            if let Some(value) = payload.get(payload_key).and_then(serde_json::Value::as_str) {
                decision.fields.insert(
                    field_key.to_string(),
                    serde_json::Value::String(value.to_string()),
                );
            }
        }
    }
    if event == "subagent-stop"
        && let Some(session_id) =
            archive_stopped_managed_child(client, &project_root, &payload, asp_session_policy)?
    {
        decision.decision = DecisionKind::Allow;
        decision.reason_kind = ReasonKind::None;
        decision.message = if client == "codex" {
            "ASP preserved the completed managed resident as idle; allow the native child turn to finish."
                    .to_string()
        } else {
            "ASP archived the stopped managed child; allow native subagent shutdown.".to_string()
        };
        decision.fields.insert(
            "agentSessionAction".to_string(),
            serde_json::Value::String(if client == "codex" {
                "subagent-stop-preserved-resident-idle".to_string()
            } else {
                "subagent-stop-archived-managed-child".to_string()
            }),
        );
        decision.fields.insert(
            "childSessionId".to_string(),
            serde_json::Value::String(session_id),
        );
    }
    if let Err(error) = annotate_payload_context(&project_root, &mut decision, &payload) {
        eprintln!("[agent-semantic-hook] failed to annotate hook payload context: {error}");
    }
    materialize_source_access_deny_message(&mut decision, hook_config);
    if let Err(error) = apply_repeated_deny_replay(&project_root, &mut decision) {
        eprintln!("[agent-semantic-hook] failed to inspect hook replay state: {error}");
    }
    if let Err(error) = enforce_resident_child_deny_contract(
        &project_root,
        asp_session_policy,
        &payload,
        &mut decision,
    ) {
        eprintln!("[agent-semantic-hook] failed to enforce resident child deny contract: {error}");
    }
    record_active_context(ActiveContextRecord {
        activation_path: &activation_path,
        platform: client,
        event,
        payload: &payload,
        decision: &decision,
    });
    if let Err(error) = append_hook_event_state(&project_root, &decision) {
        eprintln!("[agent-semantic-hook] failed to update hook state: {error}");
    }
    emit_decision(emit, &decision)
}

fn archive_stopped_managed_child(
    platform: &str,
    project_root: &Path,
    payload: &serde_json::Value,
    asp_session_policy: &hook_runtime_agent_session::AspSessionPolicy,
) -> Result<Option<String>, String> {
    let session_id = if platform == "codex" {
        if payload
            .get("hook_event_name")
            .and_then(serde_json::Value::as_str)
            != Some("SubagentStop")
        {
            return Ok(None);
        }
        let Some(agent_type) = string_field(payload, &["agent_type", "agentType"]) else {
            return Ok(None);
        };
        if agent_type != asp_session_policy.resident_agent_role() {
            return Ok(None);
        }
        let Some(agent_id) = string_field(payload, &["agent_id", "agentId"]) else {
            return Ok(None);
        };
        agent_id
    } else {
        let Some(session_id) = string_field(
            payload,
            &[
                "child_session_id",
                "childSessionId",
                "session_id",
                "sessionId",
            ],
        ) else {
            return Ok(None);
        };
        session_id
    };
    let Some(registry) = AgentSessionRegistry::open_existing_project(project_root)? else {
        return Ok(None);
    };
    let project_id = AgentSessionRegistry::workspace_id(project_root)?;
    let Some(session) = registry.lookup_session(AgentSessionLookupRequest {
        project_id: (&project_id).into(),
        session_id: Some((&session_id).into()),
        root_session_id: None,
        name: None,
    })?
    else {
        return Ok(None);
    };
    if !hook_runtime_agent_session::session_matches_resident_agent(
        &session,
        asp_session_policy.resident_child_name(),
        asp_session_policy.resident_agent_role(),
    ) {
        return Ok(None);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("failed to read subagent-stop timestamp: {error}"))?
        .as_secs() as i64;
    // Codex emits SubagentStop when one child turn finishes, while the native
    // collaboration target remains addressable through followup_task. A
    // resident therefore becomes idle here; archiving it would confuse turn
    // completion with resident identity termination.
    let updated = if platform == "codex" {
        registry.update_session_status(&project_id, &session_id, "idle", now)?
    } else {
        registry.archive_session(&project_id, &session_id, now)?
    };
    if updated {
        return Ok(Some(session_id));
    }
    Ok(None)
}

fn resident_asp_explore_sandbox_mode() -> Option<String> {
    let Some(path) = std::env::var_os("ASP_AGENTS_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| {
                std::path::PathBuf::from(home)
                    .join(".agent-semantic-protocols")
                    .join("agents")
            })
        })
        .map(|path| path.join("asp-explorer_codex.toml"))
    else {
        return Some("read-only".to_string());
    };
    let Some(contents) = std::fs::read_to_string(path).ok() else {
        return Some("read-only".to_string());
    };
    let Some(config) = toml::from_str::<toml::Value>(&contents).ok() else {
        return Some("read-only".to_string());
    };
    config
        .get("sandbox_mode")
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .or_else(|| Some("read-only".to_string()))
}

fn annotate_payload_context(
    project_root: &Path,
    decision: &mut HookDecision,
    payload: &serde_json::Value,
) -> Result<(), String> {
    for (field, keys) in [
        ("sessionId", &["session_id", "sessionId"][..]),
        ("transcriptPath", &["transcript_path", "transcriptPath"][..]),
        ("toolUseId", &["tool_use_id", "toolUseId"][..]),
        ("cwd", &["cwd"][..]),
    ] {
        if decision.fields.contains_key(field) {
            continue;
        }
        if let Some(value) = string_field(payload, keys) {
            decision
                .fields
                .insert(field.to_string(), serde_json::Value::String(value));
        }
    }
    let subagent_context = payload_indicates_subagent_context(payload)
        || has_recorded_subagent_context(
            project_root,
            decision
                .fields
                .get("sessionId")
                .and_then(serde_json::Value::as_str)
                .map(Into::into),
            decision
                .fields
                .get("transcriptPath")
                .and_then(serde_json::Value::as_str)
                .map(Into::into),
        )
        .map_err(|error| error.to_string())?;
    if !decision.fields.contains_key("subagentContext") && subagent_context {
        decision
            .fields
            .insert("subagentContext".to_string(), serde_json::Value::Bool(true));
    }
    if decision.decision == DecisionKind::Deny
        && subagent_context
        && !hook_selected_resident_execution(decision)
    {
        decision.message = subagent_deny_message(&decision.message);
    }
    Ok(())
}

fn hook_selected_resident_execution(decision: &HookDecision) -> bool {
    decision.has_configured_resident_dispatch()
        || decision.fields.contains_key("executionLane")
            && decision
                .fields
                .get("executionTransport")
                .and_then(serde_json::Value::as_str)
                == Some("resident-agent")
}

fn enforce_resident_child_deny_contract(
    project_root: &Path,
    asp_session_policy: &hook_runtime_agent_session::AspSessionPolicy,
    payload: &serde_json::Value,
    decision: &mut HookDecision,
) -> Result<(), String> {
    if decision.decision != DecisionKind::Deny {
        return Ok(());
    }
    let payload_agent_id = ["agent_id", "agentId"]
        .iter()
        .find_map(|field| payload.get(*field).and_then(serde_json::Value::as_str));
    let payload_session_id = ["session_id", "sessionId"]
        .iter()
        .find_map(|field| payload.get(*field).and_then(serde_json::Value::as_str));
    let transcript_matches_payload_session = ["transcript_path", "transcriptPath"]
        .iter()
        .find_map(|field| payload.get(*field).and_then(serde_json::Value::as_str))
        .and_then(|path| Path::new(path).file_name())
        .and_then(|name| name.to_str())
        .zip(payload_session_id)
        .map(|(name, session_id)| name.contains(session_id));
    let codex_root_thread_id = env::var("CODEX_THREAD_ID").ok();
    if payload_agent_id.is_none()
        && payload_session_id.is_some()
        && (payload_session_id == codex_root_thread_id.as_deref()
            || transcript_matches_payload_session == Some(true))
        && transcript_matches_payload_session != Some(false)
    {
        decision.fields.insert(
            "payloadLiveTargetIdentityProofStatus".to_string(),
            serde_json::Value::String("root-hook-envelope".to_string()),
        );
        return Ok(());
    }
    let configured_resident_name = decision
        .fields
        .get("residentChildName")
        .or_else(|| decision.fields.get("residentName"))
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| asp_session_policy.resident_child_name())
        .to_string();
    let configured_resident_role = decision
        .fields
        .get("targetAgentRole")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| asp_session_policy.resident_agent_role())
        .to_string();
    let configured_resident_agent_name = decision
        .fields
        .get("targetAgentName")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| asp_session_policy.resident_codex_agent_name())
        .to_string();
    for (decision_field, payload_fields) in [
        ("codexHookAgentId", ["agent_id", "agentId"]),
        ("codexHookAgentType", ["agent_type", "agentType"]),
    ] {
        if let Some(value) = payload_fields
            .iter()
            .find_map(|field| payload.get(*field).and_then(serde_json::Value::as_str))
        {
            decision.fields.insert(
                decision_field.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    let mut identity_proof =
        hook_runtime_agent_session::current_session_configured_resident_identity_proof(
            project_root,
            payload,
            &configured_resident_name,
            &configured_resident_role,
            &configured_resident_agent_name,
        )?;
    let root_session_id = decision
        .fields
        .get("sessionId")
        .and_then(serde_json::Value::as_str)
        .filter(|session_id| !session_id.trim().is_empty())
        .map(str::to_string);
    if identity_proof.is_none()
        && let Some(root_session_id) = root_session_id.as_deref()
    {
        identity_proof =
            crate::command::agent_session_registry::payload_live_target_resident_identity_proof(
                project_root,
                payload_agent_id,
                root_session_id,
                &configured_resident_name,
                &configured_resident_role,
                &configured_resident_agent_name,
            )?;
        if identity_proof.is_none() {
            let status =
                crate::command::agent_session_registry::payload_live_target_resident_identity_status(
                    project_root,
                    payload_agent_id,
                    root_session_id,
                    &configured_resident_name,
                    &configured_resident_role,
                    &configured_resident_agent_name,
                )?;
            decision.fields.insert(
                "payloadLiveTargetIdentityProofStatus".to_string(),
                serde_json::Value::String(status.to_string()),
            );
        }
    }
    if identity_proof.is_none() {
        return Ok(());
    }
    let serialized_reason_kind = serde_json::to_value(&*decision).ok().and_then(|value| {
        value
            .get("reasonKind")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
    });
    let parser_owned_stage = decision
        .fields
        .get("blockedAspStage")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|stage| matches!(stage, "search" | "query" | "-search" | "-query"))
        || decision
            .fields
            .get("aspCommandRoute")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|route| route.starts_with("search") || route.starts_with("query"));
    let resident_parser_owned_search = identity_proof.is_some()
        && serialized_reason_kind.as_deref() == Some("asp-reasoning-routed")
        && parser_owned_stage;
    let configured_resident_dispatch = identity_proof.is_some()
        && decision
            .fields
            .get("agentSessionAction")
            .and_then(serde_json::Value::as_str)
            == Some("dispatch-configured-resident")
        && decision
            .fields
            .get("residentName")
            .and_then(serde_json::Value::as_str)
            == Some(configured_resident_name.as_str())
        && decision
            .fields
            .get("receiptKind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty());
    for field in [
        "requiredAction",
        "nextAction",
        "agentSessionLoopCommand",
        "agentSessionBootstrap",
        "agentSessionBootstrapGuideCommand",
        "agentSessionBootstrapCommand",
    ] {
        decision.fields.remove(field);
    }
    decision
        .fields
        .insert("subagentContext".to_string(), serde_json::Value::Bool(true));
    let reason = serde_json::to_value(decision.reason_kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "source-access".to_string());
    let recovery_ref = decision
        .fields
        .get("recoveryRef")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("resident-child-direct-route");
    let compact_message = format!(
        "ASP denied source access (`{reason}`) inside `{configured_resident_name}`. Next: execute the selected parser-owned ASP route and return `asp.search.playbook-receipt`. recoveryRef={recovery_ref}"
    );
    decision.fields.insert(
        "registeredResidentChild".to_string(),
        serde_json::Value::Bool(matches!(
            identity_proof,
            Some(
                crate::command::ResidentChildIdentityProof::RegistryExact
                    | crate::command::ResidentChildIdentityProof::CodexTranscriptRegistryExact
                    | crate::command::ResidentChildIdentityProof::CodexHookPayloadLiveTarget
            )
        )),
    );
    if let Some(identity_proof) = identity_proof {
        decision.fields.insert(
            "residentChildIdentityProof".to_string(),
            serde_json::Value::String(
                match identity_proof {
                    crate::command::ResidentChildIdentityProof::CodexHookPayload => {
                        "codex-hook-payload"
                    }
                    crate::command::ResidentChildIdentityProof::RegistryExact => "registry-exact",
                    crate::command::ResidentChildIdentityProof::CodexTranscriptRegistryExact => {
                        "codex-transcript-registry-exact"
                    }
                    crate::command::ResidentChildIdentityProof::CodexRolloutMetadata => {
                        "codex-rollout-metadata"
                    }
                    crate::command::ResidentChildIdentityProof::CodexHookPayloadLiveTarget => {
                        "codex-hook-payload-live-target"
                    }
                }
                .to_string(),
            ),
        );
    }
    if resident_parser_owned_search || configured_resident_dispatch {
        decision.decision = DecisionKind::Allow;
        decision.reason_kind = ReasonKind::None;
        decision.fields.insert(
            "agentSessionAction".to_string(),
            serde_json::Value::String("active-hook-selected-resident".to_string()),
        );
        decision.fields.insert(
            "executionLane".to_string(),
            serde_json::Value::String(
                if configured_resident_name == "asp-testing" {
                    "testing"
                } else {
                    "search"
                }
                .to_string(),
            ),
        );
        hook_runtime_agent_session::append_terminal_execution_fields(
            &mut decision.fields,
            "active-hook-selected-resident",
        );
        decision.fields.insert(
            "residentChildConfiguredCommand".to_string(),
            serde_json::Value::Bool(true),
        );
        decision.message = "Registered resident child may execute the command selected for its configured profile directly."
            .to_string();
        return Ok(());
    }
    decision.message = compact_message;
    Ok(())
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn activation_relative_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let configured = PathBuf::from(project_root);
    let root = if configured.is_absolute() {
        configured
    } else {
        activation_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(configured)
    };
    fs::canonicalize(&root).unwrap_or(root)
}

fn hook_runtime_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let activation_root = activation_relative_project_root(activation_path, project_root);
    if activation_root_is_global_hook_state(activation_path, &activation_root) {
        if let Some(project_root) = hook_runtime_project_root_override() {
            return project_root;
        }
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        return fs::canonicalize(&cwd).unwrap_or(cwd);
    }
    activation_root
}

fn activation_repair_project_root(
    payload: &serde_json::Value,
    activation_path: &Path,
) -> Result<PathBuf, String> {
    let payload_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .map(|root| fs::canonicalize(&root).unwrap_or(root));
    let activation_owner =
        agent_semantic_runtime::state::project_root_for_activation_path(activation_path)
            .map(|root| fs::canonicalize(&root).unwrap_or(root));

    if let Some(owner) = activation_owner {
        if let Some(payload_root) = payload_root
            && !payload_root.starts_with(&owner)
        {
            return Err(format!(
                "identity/state mismatch: activationOwner={} payloadCwd={}",
                owner.display(),
                payload_root.display()
            ));
        }
        return Ok(owner);
    }
    if let Some(payload_root) = payload_root {
        return Ok(payload_root);
    }
    let current = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    Ok(fs::canonicalize(&current).unwrap_or(current))
}

fn activation_root_is_global_hook_state(activation_path: &Path, activation_root: &Path) -> bool {
    let Some(activation_dir) = activation_path.parent() else {
        return false;
    };
    if fs::canonicalize(activation_dir).unwrap_or_else(|_| activation_dir.to_path_buf())
        != fs::canonicalize(activation_root).unwrap_or_else(|_| activation_root.to_path_buf())
    {
        return false;
    }
    activation_dir.file_name().and_then(|name| name.to_str()) == Some("state")
        && activation_dir.ancestors().any(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "hooks")
        })
}

fn project_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let root = positionals(args)
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::canonicalize(&root)
        .map_err(|error| format!("failed to resolve project root {}: {error}", root.display()))
}

fn ensure_supported_client(client: &str) -> Result<(), String> {
    if matches!(client, "codex" | "claude") {
        Ok(())
    } else {
        Err(format!(
            "unsupported --client {client}; expected codex or claude"
        ))
    }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn first_positional(args: &[String]) -> Option<&str> {
    positionals(args).first().copied()
}

fn positionals(args: &[String]) -> Vec<&str> {
    let mut skip_next = false;
    let mut values = Vec::new();
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if matches!(
            arg.as_str(),
            "--client" | "--activation" | "--config" | "--emit" | "--output" | "--subagent-model"
        ) {
            skip_next = true;
            continue;
        }
        if !arg.starts_with('-') {
            values.push(arg.as_str());
        }
    }
    values
}
