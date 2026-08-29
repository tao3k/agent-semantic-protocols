//! Runtime for the `asp hook` command surface.

#[path = "hook_enablement_acceptance.rs"]
mod hook_enablement_acceptance;
#[path = "hook_runtime_cli_args.rs"]
mod hook_runtime_cli_args;
#[path = "hook_runtime_codex_plugin.rs"]
mod hook_runtime_codex_plugin;
#[path = "hook_runtime_decision_render.rs"]
mod hook_runtime_decision_render;
#[path = "hook_runtime_doctor.rs"]
mod hook_runtime_doctor;
#[path = "hook_runtime_failure.rs"]
mod hook_runtime_failure;
#[path = "hook_runtime_host_lifecycle.rs"]
mod hook_runtime_host_lifecycle;
#[path = "hook_runtime_install.rs"]
mod hook_runtime_install;
#[path = "hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "hook_runtime_stdin.rs"]
mod hook_runtime_stdin;
#[path = "hook_runtime_subagent.rs"]
mod hook_runtime_subagent;
#[path = "hook_runtime_workspace_mutation.rs"]
mod hook_runtime_workspace_mutation;

#[cfg(test)]
use super::payload_indicates_subagent_context;
use agent_semantic_hook::parse_payload;
use agent_semantic_runtime::project_state_paths;
use hook_runtime_cli_args::{display_path, optional_flag_value};
use hook_runtime_decision_render::emit_hook_runtime_failure;
use hook_runtime_doctor::run_doctor;
pub(super) use hook_runtime_install::run_codex_plugin_install_args;
use hook_runtime_install::run_install;
pub(crate) fn read_hook_input_bounded() -> Result<String, String> {
    hook_runtime_stdin::read_hook_stdin_bounded()
        .map_err(|error| format!("failed to read hook payload from stdin: {error}"))
}
use agent_semantic_hook::hook_workspace_candidate;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) async fn run_hook_runtime_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run(args.into_iter().map(Into::into).collect()).await
}

async fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("accept-host") => super::hook_host_acceptance::run_accept_host(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]).await,
        Some("enablement") => hook_enablement_acceptance::run(&args[1..]).await,
        Some("install") => run_install(&args[1..]).await,
        Some("paths") => run_paths(&args[1..]),
        _ => {
            Err("usage: asp hook <accept-host|doctor|enablement|paths> --client codex".to_string())
        }
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
    let event = first_positional(args).map(str::to_owned);
    let client = flag_value(args, "--client").map(str::to_owned);
    let execution = hook_runtime_failure::observe_hook_execution(
        event,
        client,
        run_hook_from_bootstrap_inner(args, stdin),
    );
    if tokio::runtime::Handle::try_current().is_ok() {
        return execution.await;
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("failed to create Hook Tokio runtime: {error}"))?
        .block_on(execution)
}

async fn run_hook_from_bootstrap_inner(args: &[String], stdin: String) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    ensure_supported_client(client)?;
    let emit = flag_value(args, "--emit").unwrap_or("platform");
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    if event == "pre-tool" {
        let host_matcher = flag_value(args, "--host-match")
            .ok_or_else(|| "missing required --host-match <matcher>".to_owned())?;
        let receipt = agent_semantic_hook::evaluate_payload_from_current(&stdin, host_matcher)?
            .unwrap_or_else(|| serde_json::json!({}));
        println!("{receipt}");
        return Ok(());
    }
    if event == "permission-request" {
        println!(
            "{}",
            serde_json::json!({
                "schemaId": "agent.semantic-protocols.hook.decision",
                "schemaVersion": 1,
                "decision": "deny",
                "reasonKind": "permission-request-denied",
                "terminal": "permission-request-denied",
                "processLaunched": false,
            })
        );
        return Ok(());
    }
    run_hook_with_input(args, client, event, emit, stdin).await
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
    let (Some(root_session_id), Some(parent_session_id), Some(agent_role)) = (
        metadata.root_session_id(),
        metadata.parent_thread_id(),
        metadata.agent_role(),
    ) else {
        return;
    };
    if !codex_rollout_is_subagent(
        metadata.session_id().as_str(),
        root_session_id.as_str(),
        parent_session_id.as_str(),
    ) {
        return;
    }
    apply_codex_subagent_context(
        payload,
        metadata.session_id().as_str(),
        root_session_id.as_str(),
        Some(parent_session_id.as_str()),
        agent_role,
    );
}

fn codex_rollout_is_subagent(
    session_id: &str,
    root_session_id: &str,
    parent_session_id: &str,
) -> bool {
    !session_id.trim().is_empty()
        && !root_session_id.trim().is_empty()
        && !parent_session_id.trim().is_empty()
        && session_id != root_session_id
        && session_id != parent_session_id
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

fn enrich_registered_host_agent_roles(
    client: &str,
    project_root: &Path,
    payload: &mut serde_json::Value,
) -> Result<(), String> {
    if client != "codex" {
        return Ok(());
    }
    let Some(agent_name) = string_field(payload, &["agent_type", "agentType"]) else {
        return Ok(());
    };
    let Some(roles) = hook_runtime_host_lifecycle::registered_host_agent_roles(
        project_root,
        client,
        agent_name.as_str(),
    )?
    else {
        return Ok(());
    };
    let Some(object) = payload.as_object_mut() else {
        return Ok(());
    };
    object.insert(
        "agent_roles".to_owned(),
        serde_json::Value::Array(roles.into_iter().map(serde_json::Value::String).collect()),
    );
    Ok(())
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
    let schema_is_current = receipt.get("schemaId").and_then(serde_json::Value::as_str)
        == Some(crate::multi_agent_session::CHILD_REGISTRATION_SCHEMA_ID)
        && receipt
            .get("schemaVersion")
            .and_then(serde_json::Value::as_u64)
            == Some(1);
    if !schema_is_current {
        return Err("child-session-registration-receipt-schema-invalid".to_owned());
    }
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
        .filter(|authority| *authority == crate::multi_agent_session::CHILD_REGISTRATION_AUTHORITY)
        .ok_or_else(|| "child-session-registration-receipt-authority-invalid".to_owned())?;
    let host_receipt_digest = receipt
        .get("hostCallReceiptDigest")
        .and_then(serde_json::Value::as_str)
        .filter(|digest| {
            digest.strip_prefix("blake3-256:").is_some_and(|hex| {
                hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        })
        .ok_or_else(|| "child-session-registration-receipt-host-digest-invalid".to_owned())?;
    let registered_child_session_id = receipt
        .get("childSessionId")
        .and_then(serde_json::Value::as_str)
        .filter(|session_id| !session_id.trim().is_empty())
        .ok_or_else(|| "child-session-registration-receipt-child-id-invalid".to_owned())?;
    let registered_agent_name = receipt
        .get("canonicalAgentName")
        .and_then(serde_json::Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "child-session-registration-receipt-agent-name-invalid".to_owned())?;
    let _registered_host_role = receipt
        .get("hostRole")
        .and_then(serde_json::Value::as_str)
        .filter(|role| !role.trim().is_empty())
        .ok_or_else(|| "child-registration-receipt-schema-stale-reregister-required".to_owned())?;
    let _registered_platform = receipt
        .get("platform")
        .and_then(serde_json::Value::as_str)
        .filter(|platform| !platform.trim().is_empty())
        .ok_or_else(|| "child-session-registration-receipt-platform-invalid".to_owned())?;
    for field in ["routeDigest", "profileDigest", "policyDigest"] {
        receipt
            .get(field)
            .and_then(serde_json::Value::as_str)
            .filter(|digest| {
                digest.strip_prefix("blake3-256:").is_some_and(|hex| {
                    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
            })
            .ok_or_else(|| format!("child-session-registration-receipt-{field}-invalid"))?;
    }
    let definition_schema_id = receipt
        .get("definitionSchemaId")
        .and_then(serde_json::Value::as_str)
        .filter(|schema_id| {
            *schema_id
                == agent_semantic_config::agent_route_registry::CODEX_AGENT_DEFINITION_SCHEMA_ID
        })
        .ok_or_else(|| "child-session-registration-receipt-schema-invalid".to_owned())?;
    let allowed_rule_intents = receipt
        .get("allowedRuleIntents")
        .and_then(serde_json::Value::as_array)
        .filter(|intents| !intents.is_empty())
        .ok_or_else(|| "child-session-registration-receipt-scope-invalid".to_owned())?;
    let mut unique_intents = std::collections::BTreeSet::new();
    if allowed_rule_intents.iter().any(|intent| {
        intent
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .is_none_or(|value| !unique_intents.insert(value))
    }) {
        return Err("child-session-registration-receipt-scope-invalid".to_owned());
    }
    let denied_actions = receipt
        .get("deniedActions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "child-session-registration-receipt-permissions-invalid".to_owned())?;
    let owner_scoped_mutation = unique_intents.contains("owner-scoped-mutation");
    let permissions_match_intent = if owner_scoped_mutation {
        unique_intents.len() == 1 && denied_actions.is_empty()
    } else {
        denied_actions.len() == 1 && denied_actions[0].as_str() == Some("edit")
    };
    if !permissions_match_intent {
        return Err("child-session-registration-receipt-permissions-invalid".to_owned());
    }
    if !lifecycle_is_live || !routable {
        return Err("child-session-registration-receipt-not-routable".to_owned());
    }
    let payload_child_session_id = payload
        .get("agent_id")
        .or_else(|| payload.get("agentId"))
        .and_then(serde_json::Value::as_str);
    if payload_child_session_id != Some(registered_child_session_id) {
        return Err("child-session-registration-receipt-payload-binding-mismatch".to_owned());
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
    object.insert(
        "registered_agent_name".to_owned(),
        serde_json::Value::String(registered_agent_name.to_owned()),
    );
    for (payload_key, receipt_key) in [
        ("root_session_id", "rootSessionId"),
        ("parent_session_id", "parentSessionId"),
        ("child_session_id", "childSessionId"),
    ] {
        let value = receipt
            .get(receipt_key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("child-session-registration-receipt-{payload_key}-invalid"))?;
        object.insert(
            payload_key.to_owned(),
            serde_json::Value::String(value.to_owned()),
        );
    }
    object.insert(
        "registered_definition_schema_id".to_owned(),
        serde_json::Value::String(definition_schema_id.to_owned()),
    );
    object.insert(
        "registered_denied_actions".to_owned(),
        serde_json::Value::Array(denied_actions.clone()),
    );
    object.insert(
        "registered_allowed_rule_intents".to_owned(),
        serde_json::Value::Array(allowed_rule_intents.clone()),
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
    stdin: String,
) -> Result<(), String> {
    let hook_started = std::time::Instant::now();
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
    let _ = args;
    enrich_codex_subagent_context(client, &mut payload);
    let payload_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let project_root = hook_workspace_candidate(&payload, &payload_root);
    enrich_registered_host_agent_roles(client, &project_root, &mut payload)?;
    let registration_receipt = if matches!(event, "pre-tool" | "permission-request" | "post-tool") {
        crate::multi_agent_session::read_child_session_registration_from_host_payload(
            &project_root,
            client,
            &payload,
        )
        .await?
    } else {
        None
    };
    if let Some(receipt_json) = registration_receipt.as_deref() {
        apply_verified_child_registration_context(&mut payload, receipt_json)?;
    }
    if event == "post-tool"
        && agent_semantic_hook::host_native_handoff::publish_from_post_tool_payload(&payload)?
            .is_some()
    {
        println!("{{}}");
        return Ok(());
    }
    if event != "pre-tool" {
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
            Ok(hook_runtime_host_lifecycle::HostLifecycleDisposition::Recorded {
                registered_session,
                register_child,
            }) => {
                if register_child {
                    crate::multi_agent_session::register_child_session_from_host_payload(
                        &project_root,
                        client,
                        &payload,
                        registered_session,
                    )
                    .await?;
                }
                println!("{{}}");
                return Ok(());
            }
            Ok(hook_runtime_host_lifecycle::HostLifecycleDisposition::NotLifecycle) => {}
            Err(error) => {
                emit_hook_runtime_failure(client, event, emit, &error)?;
                return Ok(());
            }
        }
    }
    trace_stage("observational-event-complete");
    println!("{{}}");
    Ok(())
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
                | "--host-match"
                | "--host-match-prefix"
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
