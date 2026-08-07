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
#[path = "hook_runtime_performance_failure.rs"]
mod hook_runtime_performance_failure;
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

#[cfg(test)]
#[path = "../../tests/unit/command/hook_workspace_candidate.rs"]
mod hook_workspace_candidate_tests;

use super::{codex_enforcement_report, payload_indicates_subagent_context};
use agent_semantic_hook::{
    HookClassificationRequest, HookDecision, apply_repeated_deny_replay, classify_hook_with_config,
    default_client_config_path, parse_payload,
};
use agent_semantic_runtime::project_state_paths;
use hook_runtime_cli_args::{display_path, optional_flag_value};
use hook_runtime_decision_render::{emit_decision, emit_hook_runtime_failure};
use hook_runtime_doctor::run_doctor;
pub(super) use hook_runtime_install::run_codex_plugin_install_args;
use hook_runtime_install::run_install;
use hook_runtime_source_access_materialize::materialize_source_access_deny_message;
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

async fn run_hook_with_input(
    args: &[String],
    client: &str,
    event: &str,
    emit: &str,
    classification_event: &str,
    stdin: String,
) -> Result<(), String> {
    let hook_started = std::time::Instant::now();
    let trace_stage = |stage: &str| {
        if std::env::var_os("ASP_HOOK_BOOTSTRAP_TRACE").is_some() {
            eprintln!(
                "[asp-hook] route=local-policy-evaluator stage={stage} elapsedMicros={}",
                hook_started.elapsed().as_micros()
            );
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
    let payload_root = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let payload_root = fs::canonicalize(&payload_root).unwrap_or(payload_root);
    let project_root = hook_workspace_candidate(&payload, &payload_root);
    let project_root = fs::canonicalize(&project_root).unwrap_or(project_root);
    // Hook startup is a best-effort global lifecycle ensure. It never derives
    // daemon identity from the payload workspace and never overrides an
    // explicit operator stop.
    if let Ok(state_home) = crate::server::runtime_server::state_home()
        && let Err(error) =
            crate::server::runtime_server_supervisor::ensure_runtime_server_for_hook(&state_home)
    {
        eprintln!("[agent-semantic-hook] Runtime Server ensure unavailable: {error}");
    }
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
        Ok(hook_runtime_host_lifecycle::HostLifecycleDisposition::Denied(decision)) => {
            return emit_decision(emit, &decision);
        }
        Err(error) => {
            emit_hook_runtime_failure(client, event, emit, &error)?;
            return Ok(());
        }
    }
    hook_runtime_performance_failure::relay_post_tool_wall_failure(event, &payload, &project_root)?;
    let mut runtime = agent_semantic_hook::HookRuntime {
        project_root: project_root.display().to_string(),
        rankers: Vec::new(),
        providers: Vec::new(),
    };
    let config_path = flag_value(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_client_config_path(&project_root.to_string_lossy()));
    let (loaded_hook_config, hook_matcher_generation_status) =
        hook_runtime_config_recovery::load_fresh_hook_config(&config_path, &project_root)?;
    trace_stage("config-loaded");
    let hook_config = &loaded_hook_config.config;
    hook_runtime_config_recovery::apply_language_provider_projection(
        hook_config,
        &mut runtime,
        &config_path,
    )?;
    let mut decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: hook_config,
        platform: client,
        event: classification_event,
        payload: &payload,
    });
    trace_stage("classified");
    decision.fields.insert(
        "hookMatcherGeneration".to_owned(),
        serde_json::Value::String(hook_matcher_generation_status.to_owned()),
    );
    decision.event = event.to_string();
    annotate_payload_context(&mut decision, &payload);
    materialize_source_access_deny_message(&mut decision);
    if let Err(error) = apply_repeated_deny_replay(&project_root, &mut decision) {
        eprintln!("[agent-semantic-hook] failed to inspect hook replay state: {error}");
    }
    hook_runtime_agent_session_dispatch::materialize_org_choice_plane_reference(&mut decision);
    let decision_elapsed_micros =
        u64::try_from(hook_started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let decision_budget_micros = 1_000_000_u64;
    decision.fields.insert(
        "hookDecisionElapsedMicros".to_owned(),
        serde_json::json!(decision_elapsed_micros),
    );
    decision.fields.insert(
        "hookDecisionBudgetMicros".to_owned(),
        serde_json::json!(decision_budget_micros),
    );
    decision.fields.insert(
        "hookDecisionBudgetStatus".to_owned(),
        serde_json::Value::String(
            if decision_elapsed_micros <= decision_budget_micros {
                "within-budget"
            } else {
                "budget-exceeded"
            }
            .to_owned(),
        ),
    );
    decision.fields.insert(
        "hookEventProjectionStatus".to_owned(),
        serde_json::Value::String("recorded".to_owned()),
    );
    let event_projection_started = std::time::Instant::now();
    // The decision plane is authoritative; the JSONL event file is only a
    // diagnostic projection. A contended or damaged telemetry sink must never
    // suppress an already-computed allow/deny decision.
    if let Err(error) = agent_semantic_hook::try_append_hook_event_state(&project_root, &decision) {
        decision.fields.insert(
            "hookEventProjectionStatus".to_owned(),
            serde_json::Value::String("unavailable".to_owned()),
        );
        eprintln!(
            "[agent-semantic-hook] event projection unavailable; decision remains authoritative: {error}"
        );
    }
    decision.fields.insert(
        "hookEventProjectionLockWaitMicros".to_owned(),
        serde_json::json!(
            u64::try_from(event_projection_started.elapsed().as_micros()).unwrap_or(u64::MAX)
        ),
    );
    trace_stage("complete");
    emit_decision(emit, &decision)
}

fn annotate_payload_context(decision: &mut HookDecision, payload: &serde_json::Value) {
    annotate_host_root_session_identity(
        &mut decision.fields,
        std::env::var("CODEX_THREAD_ID").ok().as_deref(),
    );
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
