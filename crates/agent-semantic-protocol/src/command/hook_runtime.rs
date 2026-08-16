//! Runtime for the `asp hook` command surface.

#[path = "hook_runtime_agent_session_dispatch.rs"]
mod hook_runtime_agent_session_dispatch;
#[path = "hook_runtime_cli_args.rs"]
mod hook_runtime_cli_args;
#[path = "hook_runtime_codex_plugin.rs"]
mod hook_runtime_codex_plugin;
#[path = "hook_runtime_config_recovery.rs"]
mod hook_runtime_config_recovery;
#[path = "hook_runtime_decision_render.rs"]
mod hook_runtime_decision_render;
#[path = "hook_runtime_doctor.rs"]
mod hook_runtime_doctor;
#[path = "hook_runtime_host_lifecycle.rs"]
mod hook_runtime_host_lifecycle;
#[path = "hook_runtime_install.rs"]
mod hook_runtime_install;
#[path = "hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "hook_runtime_source_access_materialize.rs"]
mod hook_runtime_source_access_materialize;
#[path = "hook_runtime_stdin.rs"]
mod hook_runtime_stdin;
#[path = "hook_runtime_subagent.rs"]
mod hook_runtime_subagent;
#[path = "hook_runtime_workspace_candidate.rs"]
mod hook_runtime_workspace_candidate;
#[path = "hook_runtime_workspace_mutation.rs"]
mod hook_runtime_workspace_mutation;

#[cfg(test)]
#[path = "../../tests/unit/command/hook_workspace_candidate.rs"]
mod hook_workspace_candidate_tests;

use super::{codex_enforcement_report, payload_indicates_subagent_context};
use agent_semantic_hook::{
    HookClassificationRequest, HookDecision, classify_hook_with_config, default_client_config_path,
    parse_payload,
};
use agent_semantic_runtime::project_state_paths;
use hook_runtime_cli_args::{display_path, optional_flag_value};
use hook_runtime_decision_render::{emit_decision, emit_hook_runtime_failure};
use hook_runtime_doctor::run_doctor;
pub(super) use hook_runtime_install::run_codex_plugin_install_args;
use hook_runtime_install::run_install;
use hook_runtime_source_access_materialize::materialize_source_access_deny_message;
const HOOK_DECISION_BUDGET_MICROS: u64 = 1_000;

pub(crate) fn publish_hook_matcher_generation(
    config_path: &std::path::Path,
    project_root: &std::path::Path,
) -> Result<String, String> {
    hook_runtime_config_recovery::publish_hook_matcher_generation(config_path, project_root)
}

fn current_thread_cpu_micros() -> Option<u64> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `value` is valid writable storage and the clock call does not
    // retain the pointer. Failure is represented as an unavailable sample.
    if unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut value) } != 0 {
        return None;
    }
    let seconds = u64::try_from(value.tv_sec).ok()?;
    let nanos = u64::try_from(value.tv_nsec).ok()?;
    seconds.checked_mul(1_000_000)?.checked_add(nanos / 1_000)
}

fn annotate_hook_decision_budget(
    decision: &mut HookDecision,
    started: std::time::Instant,
    cpu_started_micros: Option<u64>,
) {
    let elapsed_micros = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let cpu_micros = cpu_started_micros
        .zip(current_thread_cpu_micros())
        .and_then(|(start, end)| end.checked_sub(start));
    let budget_micros = cpu_micros.unwrap_or(elapsed_micros);
    decision.fields.insert(
        "hookDecisionElapsedMicros".to_owned(),
        serde_json::json!(elapsed_micros),
    );
    decision.fields.insert(
        "hookDecisionCpuMicros".to_owned(),
        cpu_micros.map_or(serde_json::Value::Null, serde_json::Value::from),
    );
    decision.fields.insert(
        "hookDecisionBudgetBasis".to_owned(),
        serde_json::Value::String(
            if cpu_micros.is_some() {
                "thread-cpu"
            } else {
                "wall-fallback"
            }
            .to_owned(),
        ),
    );
    decision.fields.insert(
        "hookDecisionBudgetMicros".to_owned(),
        serde_json::json!(HOOK_DECISION_BUDGET_MICROS),
    );
    decision.fields.insert(
        "hookDecisionBudgetStatus".to_owned(),
        serde_json::Value::String(
            if budget_micros < HOOK_DECISION_BUDGET_MICROS {
                "within-budget"
            } else {
                "budget-exceeded"
            }
            .to_owned(),
        ),
    );
}

pub(crate) fn read_hook_input_bounded() -> Result<String, String> {
    hook_runtime_stdin::read_hook_stdin_bounded()
        .map_err(|error| format!("failed to read hook payload from stdin: {error}"))
}
use hook_runtime_workspace_candidate::hook_workspace_candidate;
use std::fs;
use std::path::PathBuf;

pub(super) fn run_hook_runtime_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run(args.into_iter().map(Into::into).collect())
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("accept-host") => super::hook_host_acceptance::run_accept_host(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]),
        Some("install") => run_install(&args[1..]),
        Some("paths") => run_paths(&args[1..]),
        _ => Err(
            "usage: asp hook <accept-host|install|doctor|paths|hook> --client codex".to_string(),
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

pub(crate) async fn run_hook_from_bootstrap(args: &[String], stdin: String) -> Result<(), String> {
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
    run_hook_with_input(args, client, event, emit, classification_event, stdin).await
}

fn enrich_codex_subagent_context(client: &str, payload: &mut serde_json::Value) {
    if client != "codex" || payload_has_complete_typed_agent_identity(payload) {
        return;
    }
    let Some(transcript_path) = string_field(payload, &["transcript_path", "transcriptPath"])
    else {
        return;
    };
    let Ok(Some(metadata)) = agent_semantic_runtime::codex_rollout_session_metadata_at_path(
        std::path::Path::new(transcript_path.as_str()),
    ) else {
        return;
    };
    let (Some(root_session_id), Some(agent_role)) =
        (metadata.root_session_id(), metadata.agent_role())
    else {
        return;
    };
    apply_codex_subagent_context(
        payload,
        metadata.session_id().as_str(),
        root_session_id.as_str(),
        metadata.parent_thread_id().map(|value| value.as_str()),
        agent_role,
    );
}

fn payload_has_complete_typed_agent_identity(payload: &serde_json::Value) -> bool {
    string_field(payload, &["agent_id", "agentId"])
        .filter(|value| !value.trim().is_empty())
        .zip(
            string_field(payload, &["agent_type", "agentType"])
                .filter(|value| !value.trim().is_empty()),
        )
        .zip(
            string_field(payload, &["root_session_id", "rootSessionId"])
                .filter(|value| !value.trim().is_empty()),
        )
        .is_some()
}

fn apply_codex_subagent_context(
    payload: &mut serde_json::Value,
    session_id: &str,
    root_session_id: &str,
    parent_session_id: Option<&str>,
    agent_role: &str,
) {
    let Some(object) = payload.as_object_mut() else {
        return;
    };
    object
        .entry("is_subagent".to_owned())
        .or_insert(serde_json::Value::Bool(true));
    insert_missing_or_blank(object, "agent_id", session_id);
    insert_missing_or_blank(object, "agent_type", agent_role);
    insert_missing_or_blank(object, "root_session_id", root_session_id);
    insert_missing_or_blank(
        object,
        "parent_session_id",
        parent_session_id.unwrap_or(root_session_id),
    );
}

fn insert_missing_or_blank(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    value: &str,
) {
    if object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|current| !current.trim().is_empty())
    {
        return;
    }
    object.insert(
        field.to_owned(),
        serde_json::Value::String(value.to_owned()),
    );
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_subagent_context.rs"]
mod hook_runtime_subagent_context_tests;

fn apply_verified_child_registration_context(
    payload: &mut serde_json::Value,
    receipt_json: &str,
) -> Result<(), String> {
    let receipt: serde_json::Value = serde_json::from_str(receipt_json)
        .map_err(|error| format!("child-session-registration-receipt-invalid: {error}"))?;
    let generation = receipt
        .get("generation")
        .and_then(serde_json::Value::as_u64)
        .filter(|generation| *generation > 0)
        .ok_or_else(|| "child-session-registration-receipt-generation-invalid".to_owned())?;
    let lifecycle_is_live = receipt
        .get("lifecycleState")
        .and_then(serde_json::Value::as_str)
        == Some("live");
    let routable = receipt
        .get("routable")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let authority = receipt
        .get("registrationAuthority")
        .and_then(serde_json::Value::as_str)
        .filter(|authority| !authority.trim().is_empty())
        .ok_or_else(|| "child-session-registration-receipt-authority-invalid".to_owned())?;
    let host_receipt_digest = receipt
        .get("hostCallReceiptDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| digest.starts_with("blake3-256:"))
        .ok_or_else(|| "child-session-registration-receipt-host-digest-invalid".to_owned())?;
    if !lifecycle_is_live || !routable {
        return Err("child-session-registration-receipt-not-routable".to_owned());
    }
    let object = payload
        .as_object_mut()
        .ok_or_else(|| "hook payload must be an object".to_owned())?;
    object.insert(
        "registration_verified".to_owned(),
        serde_json::Value::Bool(true),
    );
    object.insert(
        "registration_generation".to_owned(),
        serde_json::Value::Number(generation.into()),
    );
    object.insert(
        "registration_authority".to_owned(),
        serde_json::Value::String(authority.to_owned()),
    );
    object.insert(
        "registration_host_receipt_digest".to_owned(),
        serde_json::Value::String(host_receipt_digest.to_owned()),
    );
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/hook_runtime_registration_context.rs"]
mod registration_context_tests;

async fn run_hook_with_input(
    args: &[String],
    client: &str,
    event: &str,
    emit: &str,
    classification_event: &str,
    stdin: String,
) -> Result<(), String> {
    let hook_started = std::time::Instant::now();
    let hook_cpu_started_micros = current_thread_cpu_micros();
    let trace_enabled = std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some();
    let trace_stage = |stage: &str| {
        if trace_enabled {
            eprintln!(
                "[asp-hook] route=local-policy-evaluator stage={stage} elapsedMicros={}",
                hook_started.elapsed().as_micros()
            );
        }
    };
    let mut payload = match parse_payload(&stdin) {
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
    let parsed_micros = hook_started.elapsed().as_micros();
    enrich_codex_subagent_context(client, &mut payload);
    let payload_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root = hook_workspace_candidate(&payload, &payload_root);
    let registration_receipt =
        crate::multi_agent_session::register_child_session_from_host_payload(
            &project_root,
            client,
            &payload,
        )
        .await?;
    if let Some(receipt_json) = registration_receipt.as_deref() {
        apply_verified_child_registration_context(&mut payload, receipt_json)?;
    }
    let workspace_micros = hook_started.elapsed().as_micros();
    if let Some(mut decision) =
        agent_semantic_hook::runtime_binary_policy_decision_v1(client, event, &payload)
    {
        annotate_hook_decision_budget(&mut decision, hook_started, hook_cpu_started_micros);
        return emit_decision(emit, &decision);
    }
    hook_runtime_workspace_mutation::relay_post_tool_workspace_mutation(
        event,
        &payload,
        &project_root,
    )
    .await?;
    match hook_runtime_host_lifecycle::record_host_lifecycle_event(
        client,
        event,
        &payload,
        &project_root,
    )
    .await
    {
        Ok(hook_runtime_host_lifecycle::HostLifecycleDisposition::Recorded) => return Ok(()),
        Ok(hook_runtime_host_lifecycle::HostLifecycleDisposition::NotLifecycle) => {}
        Err(error) => {
            emit_hook_runtime_failure(client, event, emit, &error)?;
            return Ok(());
        }
    }
    let local_event_micros = hook_started.elapsed().as_micros();
    let mut runtime = agent_semantic_hook::HookRuntime {
        project_root: project_root.display().to_string(),
        rankers: Vec::new(),
        providers: Vec::new(),
        policy_providers: Vec::new(),
    };
    let config_path = flag_value(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_client_config_path(&project_root.to_string_lossy()));
    let direct_read_key = agent_semantic_hook::direct_read_source_key(&payload);
    let shell_read_keys = direct_read_key
        .is_none()
        .then(|| agent_semantic_hook::shell_read_source_keys(&payload))
        .unwrap_or_default();
    let shell_command_key = direct_read_key
        .is_none()
        .then(|| agent_semantic_hook::shell_command_key(&payload))
        .flatten();
    let (loaded_hook_config, hook_matcher_generation_status) =
        hook_runtime_config_recovery::load_fresh_hook_config(
            &config_path,
            &project_root,
            direct_read_key.as_ref().map(|key| key.extension.as_str()),
            direct_read_key.as_ref().map(|key| key.path.as_str()),
            &shell_read_keys,
            shell_command_key.as_ref(),
        )?;
    let config_micros = hook_started.elapsed().as_micros();
    trace_stage("config-loaded");
    let hook_runtime_config_recovery::LoadedHookConfig {
        mut config,
        decision,
        projection,
    } = loaded_hook_config;
    let matcher_projection = projection.unwrap_or("complete-policy-matcher");
    let mut decision = if let Some(decision) = decision {
        if shell_command_key.is_some() {
            agent_semantic_hook::rebind_command_decision_to_payload(decision, &payload)
        } else {
            decision
        }
    } else {
        let hook_config = config
            .as_mut()
            .ok_or_else(|| "Hook matcher loaded neither config nor decision shard".to_owned())?;
        hook_runtime_config_recovery::move_language_provider_projection(
            hook_config,
            &mut runtime,
            &config_path,
        )?;
        classify_hook_with_config(HookClassificationRequest {
            registry: &runtime,
            config: hook_config,
            platform: client,
            event: classification_event,
            payload: &payload,
        })
    };
    if let Some(key) = &direct_read_key {
        if decision.subject.tool_name.as_deref() != Some(key.tool_name.as_str()) {
            decision.subject.tool_name = Some(key.tool_name.clone());
        }
    }
    let provider_projection_micros = hook_started.elapsed().as_micros();
    let classified_micros = hook_started.elapsed().as_micros();
    trace_stage("classified");
    decision.fields.insert(
        "hookMatcherGeneration".to_owned(),
        serde_json::Value::String(hook_matcher_generation_status.to_owned()),
    );
    decision.fields.insert(
        "hookMatcherProjection".to_owned(),
        serde_json::Value::String(matcher_projection.to_owned()),
    );
    decision.fields.insert(
        "hookPolicySynchronousDependencies".to_owned(),
        serde_json::Value::Array(Vec::new()),
    );
    if decision.event != event {
        decision.event = event.to_owned();
    }
    annotate_payload_context(&mut decision, &payload);
    if projection.is_none() {
        materialize_source_access_deny_message(&mut decision);
        hook_runtime_agent_session_dispatch::materialize_org_choice_plane_reference(&mut decision);
    }
    let materialized_micros = hook_started.elapsed().as_micros();
    if trace_enabled {
        decision.fields.insert(
            "hookStageCumulativeMicros".to_owned(),
            serde_json::json!({
                "parsed": parsed_micros,
                "workspace": workspace_micros,
                "localEvents": local_event_micros,
                "config": config_micros,
                "providerProjection": provider_projection_micros,
                "classified": classified_micros,
                "materialized": materialized_micros,
            }),
        );
    }
    annotate_hook_decision_budget(&mut decision, hook_started, hook_cpu_started_micros);
    let requires_choice_plane_route = decision
        .fields
        .get("agentWindowCommand")
        .and_then(serde_json::Value::as_str)
        == Some("asp session --agents choice-plane")
        && decision
            .fields
            .get("choicePlaneOwner")
            .and_then(serde_json::Value::as_str)
            == Some("org-contract:agent-interactive");
    if requires_choice_plane_route {
        decision.fields.insert(
            "hookEventProjectionStatus".to_owned(),
            serde_json::Value::String("project-ledger".to_owned()),
        );
        match agent_semantic_hook::try_append_hook_event_state(&project_root, &decision) {
            Ok(path) => {
                decision.fields.insert(
                    "hookEventProjectionPath".to_owned(),
                    serde_json::Value::String(path.display().to_string()),
                );
            }
            Err(error) => {
                decision.fields.insert(
                    "hookEventProjectionStatus".to_owned(),
                    serde_json::Value::String("failed".to_owned()),
                );
                decision.fields.insert(
                    "hookEventProjectionFailure".to_owned(),
                    serde_json::Value::String(error),
                );
            }
        }
    } else {
        decision.fields.insert(
            "hookEventProjectionStatus".to_owned(),
            serde_json::Value::String("out-of-band".to_owned()),
        );
    }
    trace_stage("complete");
    emit_decision(emit, &decision)
}

fn annotate_payload_context(decision: &mut HookDecision, payload: &serde_json::Value) {
    let dispatch_relevant = decision.reason_kind
        == agent_semantic_hook::ReasonKind::SubagentReceiptRequired
        || decision.fields.contains_key("targetAgentName")
        || decision.fields.contains_key("residentChildName");
    if dispatch_relevant {
        annotate_host_root_session_identity(
            &mut decision.fields,
            std::env::var("CODEX_THREAD_ID").ok().as_deref(),
        );
    }
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
    let subagent_context = payload_indicates_subagent_context(payload);
    if !decision.fields.contains_key("subagentContext") && subagent_context {
        decision
            .fields
            .insert("subagentContext".to_string(), serde_json::Value::Bool(true));
    }
}

fn annotate_host_root_session_identity(
    fields: &mut std::collections::BTreeMap<String, serde_json::Value>,
    host_root: Option<&str>,
) {
    let Some(host_root) = host_root.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    fields
        .entry("hostRootSessionId".to_string())
        .or_insert_with(|| serde_json::Value::String(host_root.to_string()));
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn project_root_arg(args: &[String]) -> Result<PathBuf, String> {
    let root = positionals(args)
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    fs::canonicalize(&root)
        .map_err(|error| format!("failed to resolve project root {}: {error}", root.display()))
}

pub(crate) fn ensure_supported_client(client: &str) -> Result<(), String> {
    if matches!(client, "codex" | "claude") {
        Ok(())
    } else {
        Err(format!(
            "unsupported --client {client}; expected codex or claude"
        ))
    }
}

pub(crate) fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

pub(crate) fn first_positional(args: &[String]) -> Option<&str> {
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
            "--client"
                | "--activation"
                | "--config"
                | "--emit"
                | "--host-probe-path"
                | "--host-rollout"
                | "--host-sentinel"
                | "--output"
                | "--subagent-model"
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
