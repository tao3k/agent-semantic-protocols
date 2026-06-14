//! Runtime for the `asp hook` command surface.

#[path = "hook_runtime_codex_plugin.rs"]
mod hook_runtime_codex_plugin;
#[path = "hook_runtime_skill.rs"]
mod hook_runtime_skill;
#[path = "hook_runtime_stdin.rs"]
mod hook_runtime_stdin;
#[path = "hook_runtime_subagent.rs"]
mod hook_runtime_subagent;

use super::{
    codex_enforcement_report, ensure_protocol_binary_installed_for_path,
    payload_indicates_subagent_context, protocol_binary_on_path,
};
use agent_semantic_hook::{
    ActiveContextRecord, DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HOOK_TRIGGER_PROMPT_FILE_NAME, HookActivation, HookClassificationRequest, HookDecision,
    ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, ReasonKind, RuntimeProviderHealthStatus,
    append_hook_event_state, apply_repeated_deny_replay, classify_hook_with_config,
    claude_hook_block, codex_user_trust_state_status, default_activation_path,
    default_claude_settings_path, default_client_config_path,
    default_client_config_template_for_source_extensions, default_hook_trigger_prompt_message,
    discover_activation_path, has_recorded_subagent_context, load_activation, load_client_config,
    load_or_refresh_default_activation, load_or_sync_activation, merge_claude_settings,
    merge_hook_trigger_prompt_document, parse_payload, record_active_context,
    remove_incompatible_hook_event_state, remove_legacy_codex_hook_cache_files,
    render_hook_trigger_prompt_document, render_platform_response, runtime_profiles_for_activation,
    runtime_profiles_for_runtime, subagent_deny_message, validate_claude_settings_json,
};
use agent_semantic_runtime::ensure_project_hook_cache_dir;
use hook_runtime_codex_plugin::{codex_plugin_scope_arg, install_codex_plugin_hooks};
use hook_runtime_skill::install_agent_semantic_protocols_skill;
use hook_runtime_stdin::read_hook_stdin_bounded;
use hook_runtime_subagent::{install_claude_asp_explorer_agent, subagent_model_arg};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn run_hook_runtime_args<I, S>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run(args.into_iter().map(Into::into).collect())
}

fn run(args: Vec<String>) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("hook") => run_hook(&args[1..]),
        Some("doctor") => run_doctor(&args[1..]),
        Some("install") => run_install(&args[1..]),
        _ => Err("usage: asp hook <install|doctor|hook> --client codex [PROJECT_ROOT]".to_string()),
    }
}

fn run_hook(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    ensure_supported_client(client)?;
    let emit = flag_value(args, "--emit").unwrap_or("platform");
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    let activation_path = flag_value(args, "--activation")
        .map(PathBuf::from)
        .unwrap_or_else(default_or_discovered_activation_path);
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
    let mut runtime = match load_activation(&activation_path) {
        Ok(registry) => registry,
        Err(error) => {
            emit_activation_load_failure(client, event, emit, &activation_path, &error, &stdin)?;
            return Ok(());
        }
    };
    let project_root = activation_relative_project_root(&activation_path, &runtime.project_root);
    runtime.project_root = project_root.display().to_string();
    let config_path = flag_value(args, "--config")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_client_config_path(&project_root.to_string_lossy()));
    let hook_config = match load_client_config(&config_path) {
        Ok(config) => config,
        Err(error) => {
            emit_hook_config_load_failure(client, event, emit, &config_path, &error)?;
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
    let mut decision = classify_hook_with_config(HookClassificationRequest {
        registry: &runtime,
        config: &hook_config,
        platform: client,
        event,
        payload: &payload,
    });
    if let Err(error) = annotate_payload_context(&project_root, &mut decision, &payload) {
        eprintln!("[agent-semantic-hook] failed to annotate hook payload context: {error}");
    }
    if let Err(error) = apply_project_hook_trigger_prompt(&project_root, client, &mut decision) {
        eprintln!("[agent-semantic-hook] failed to apply hook trigger prompt: {error}");
    }
    if let Err(error) = apply_repeated_deny_replay(&project_root, &mut decision) {
        eprintln!("[agent-semantic-hook] failed to inspect hook replay state: {error}");
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
    let output_value = match emit {
        "decision" => serde_json::to_value(&decision)
            .map_err(|error| format!("failed to serialize hook decision: {error}"))?,
        "platform" => render_platform_response(&decision)
            .map_err(|error| format!("failed to render hook response: {error:?}"))?,
        other => {
            return Err(format!(
                "unsupported --emit value: {other}; expected platform or decision"
            ));
        }
    };
    let output = serde_json::to_string(&output_value)
        .map_err(|error| format!("failed to serialize hook response: {error}"))?;
    println!("{output}");
    Ok(())
}

fn default_or_discovered_activation_path() -> PathBuf {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    discover_activation_path(&cwd).unwrap_or_else(|| default_activation_path(&PathBuf::from(".")))
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
                .and_then(serde_json::Value::as_str),
            decision
                .fields
                .get("transcriptPath")
                .and_then(serde_json::Value::as_str),
        )?;
    if !decision.fields.contains_key("subagentContext") && subagent_context {
        decision
            .fields
            .insert("subagentContext".to_string(), serde_json::Value::Bool(true));
    }
    if decision.decision == DecisionKind::Deny && subagent_context {
        decision.message = subagent_deny_message(&decision.message);
    }
    Ok(())
}

fn emit_activation_load_failure(
    client: &str,
    event: &str,
    emit: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Result<(), String> {
    eprintln!(
        "[agent-semantic-hook] activation disabled for this hook event: {}: {error}",
        activation_path.display()
    );
    if let Some(decision) =
        activation_failure_source_decision(client, event, activation_path, error, stdin)
    {
        return emit_decision(emit, &decision);
    }
    emit_hook_runtime_failure(
        client,
        event,
        emit,
        &format!(
            "Semantic hook activation could not be loaded; allowing tool use so activation can be repaired: {error}"
        ),
    )
}

fn activation_failure_source_decision(
    client: &str,
    event: &str,
    activation_path: &Path,
    error: &str,
    stdin: &str,
) -> Option<HookDecision> {
    let payload: serde_json::Value = serde_json::from_str(stdin).ok()?;
    let tool_name = string_field(&payload, &["tool_name", "toolName"]).unwrap_or_default();
    let tool_input = payload
        .get("tool_input")
        .or_else(|| payload.get("toolInput"))
        .or_else(|| payload.get("input"))
        .unwrap_or(&payload);
    let mut paths = Vec::new();
    collect_source_like_values(tool_input, &mut paths);
    let command = string_field(tool_input, &["cmd", "command", "script"]);
    if let Some(command) = command.as_deref() {
        collect_command_source_paths(command, &mut paths);
    }
    paths.sort();
    paths.dedup();
    if !paths.iter().any(|path| is_source_path(path)) {
        return None;
    }

    let is_direct_read = tool_name.eq_ignore_ascii_case("read")
        || tool_name.eq_ignore_ascii_case("view")
        || command
            .as_deref()
            .is_some_and(starts_with_source_dump_command);
    let reason_kind = if is_direct_read {
        ReasonKind::DirectSourceRead
    } else {
        ReasonKind::BulkSourceDump
    };
    Some(HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Deny,
        reason_kind,
        language_ids: Vec::new(),
        subject: DecisionSubject {
            tool_name: if tool_name.is_empty() {
                None
            } else {
                Some(tool_name)
            },
            command,
            paths,
        },
        routes: Vec::new(),
        message: format!(
            "Semantic hook activation could not be loaded from {}; source reads fail closed until activation is repaired: {error}",
            activation_path.display()
        ),
        fields: BTreeMap::new(),
    })
}

fn emit_decision(emit: &str, decision: &HookDecision) -> Result<(), String> {
    let output_value = match emit {
        "decision" => serde_json::to_value(decision)
            .map_err(|error| format!("failed to serialize hook decision: {error}"))?,
        "platform" => render_platform_response(decision)
            .map_err(|error| format!("failed to render hook response: {error:?}"))?,
        other => {
            return Err(format!(
                "unsupported --emit value: {other}; expected platform or decision"
            ));
        }
    };
    let output = serde_json::to_string(&output_value)
        .map_err(|error| format!("failed to serialize hook response: {error}"))?;
    println!("{output}");
    Ok(())
}

fn apply_project_hook_trigger_prompt(
    project_root: &Path,
    client: &str,
    decision: &mut HookDecision,
) -> Result<(), String> {
    let reason = reason_kind_label(decision.reason_kind);
    if decision.message != default_hook_trigger_prompt_message(reason, &decision.routes) {
        return Ok(());
    }
    let prompt_path = hook_trigger_prompt_path(project_root, client);
    if !prompt_path.is_file() {
        return Ok(());
    }
    let prompt = fs::read_to_string(&prompt_path)
        .map_err(|error| format!("failed to read {}: {error}", prompt_path.display()))?;
    decision.message = render_hook_trigger_prompt_document(&prompt, reason, &decision.routes);
    Ok(())
}

fn string_field(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
}

fn collect_source_like_values(value: &serde_json::Value, paths: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if is_source_path(text) => {
            paths.push(text.to_string());
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_source_like_values(value, paths);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if matches!(
                    key.as_str(),
                    "file_path" | "filePath" | "path" | "paths" | "file" | "files" | "selector"
                ) {
                    collect_source_like_values(value, paths);
                }
            }
        }
        _ => {}
    }
}

fn collect_command_source_paths(command: &str, paths: &mut Vec<String>) {
    for token in command.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '\'' | '"' | ',' | ';' | ':' | '(' | ')' | '[' | ']'
            )
        });
        if is_source_path(token) {
            paths.push(token.to_string());
        }
    }
}

fn starts_with_source_dump_command(command: &str) -> bool {
    let Some(first) = command.split_whitespace().next() else {
        return false;
    };
    matches!(
        first,
        "cat" | "sed" | "less" | "more" | "head" | "tail" | "nl" | "bat" | "awk"
    )
}

fn is_source_path(path: &str) -> bool {
    let path = path.trim();
    matches!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some(
            "rs" | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "py"
                | "pyi"
                | "jl"
                | "go"
                | "java"
                | "kt"
                | "kts"
                | "swift"
                | "c"
                | "h"
                | "cc"
                | "cpp"
                | "hpp"
        )
    )
}

fn emit_hook_config_load_failure(
    client: &str,
    event: &str,
    emit: &str,
    config_path: &Path,
    error: &str,
) -> Result<(), String> {
    eprintln!(
        "[agent-semantic-hook] blocking hook event because config failed to load: {}: {error}",
        config_path.display()
    );
    let decision = HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Block,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: format!(
            "Semantic hook config could not be loaded; blocking tool use until it is repaired: {error}"
        ),
        fields: std::collections::BTreeMap::new(),
    };
    emit_decision(emit, &decision)
}

fn emit_hook_runtime_failure(
    client: &str,
    event: &str,
    emit: &str,
    message: &str,
) -> Result<(), String> {
    let decision = HookDecision {
        schema_id: HOOK_DECISION_SCHEMA_ID,
        schema_version: HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: HOOK_PROTOCOL_ID,
        protocol_version: HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: message.to_string(),
        fields: std::collections::BTreeMap::new(),
    };
    emit_decision(emit, &decision)
}

fn run_doctor(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_supported_client(client)?;
    let project_root = project_root_arg(args)?;
    let activation_path = flag_value(args, "--activation")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_activation_path(&project_root));
    let runtime = load_or_sync_activation(&activation_path, &project_root)?;
    let runtime_profiles = runtime_profiles_for_runtime(&project_root, &runtime);
    let config_path = if client == "claude" {
        default_claude_settings_path(&project_root.to_string_lossy())
    } else {
        project_root.join(".codex").join("config.toml")
    };
    let config = fs::read_to_string(&config_path).unwrap_or_default();
    let client_config_path = default_client_config_path(&project_root.to_string_lossy());
    let hook_config = if client_config_path.is_file() {
        Some(load_client_config(&client_config_path).map_err(|error| {
            format!(
                "invalid client hook config {}: {error}",
                display_path(&project_root, &client_config_path)
            )
        })?)
    } else {
        None
    };
    let client_config_status = if hook_config.is_some() {
        "ok"
    } else {
        "missing"
    };
    let root_hook = if client == "claude" {
        config.contains("asp hook") && config.contains("--client claude")
    } else {
        config.contains(ROOT_BLOCK_BEGIN) && config.contains(ROOT_BLOCK_END)
    };
    let hook_binary_path = protocol_binary_on_path();
    let hook_binary = hook_binary_path.is_some();
    let hook_binary_path = hook_binary_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "missing".to_string());
    let enforcement = if client == "codex" {
        Some(codex_enforcement_report(
            &project_root,
            root_hook,
            hook_binary,
        ))
    } else {
        None
    };
    let (classifier_probe, classifier_reason) = if client == "codex" {
        if let Some(hook_config) = hook_config.as_ref() {
            let probe_payload = serde_json::json!({
                "tool_name": "functions.exec_command",
                "tool_input": {
                    "cmd": "sed -n '1,120p' src/lib.rs"
                }
            });
            let decision = classify_hook_with_config(HookClassificationRequest {
                registry: &runtime,
                config: hook_config,
                platform: client,
                event: "PreToolUse",
                payload: &probe_payload,
            });
            (
                decision_kind_label(decision.decision),
                reason_kind_label(decision.reason_kind),
            )
        } else {
            ("unavailable", "client-config-missing")
        }
    } else {
        ("not-applicable", "non-codex-client")
    };
    let trust_status = if client == "codex" {
        codex_user_trust_state_status(&config_path).ok()
    } else {
        None
    };
    let trust = trust_status.as_ref().is_some_and(|status| status.trusted);
    let project_trust = trust_status
        .as_ref()
        .is_some_and(|status| status.project_trusted);
    let hook_state_trust = trust_status
        .as_ref()
        .is_some_and(|status| status.hook_state_trusted);
    let trust_missing_count = trust_status
        .as_ref()
        .map(|status| status.missing_events.len())
        .unwrap_or(0);
    let trust_stale_count = trust_status
        .as_ref()
        .map(|status| status.stale_events.len())
        .unwrap_or(0);
    let trust_config = trust_status
        .as_ref()
        .map(|status| status.trust_config_path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "[agent-doctor] status=ok client={client} providers={} activation={} activationRuntime=derived config={} clientConfig={} clientConfigStatus={} hook={} trust={} projectTrust={} hookStateTrust={} trustMissing={} trustStale={} trustConfig={} binary={} binaryPath={} classifierProbe={} classifierReason={} enforcement={} enforcementProbe={} enforcementReason={} protocol={}",
        runtime.providers.len(),
        display_path(&project_root, &activation_path),
        config_path.is_file(),
        display_path(&project_root, &client_config_path),
        client_config_status,
        root_hook,
        trust,
        project_trust,
        hook_state_trust,
        trust_missing_count,
        trust_stale_count,
        trust_config,
        hook_binary,
        hook_binary_path,
        classifier_probe,
        classifier_reason,
        enforcement
            .as_ref()
            .map(|report| report.status)
            .unwrap_or("not-applicable"),
        enforcement
            .as_ref()
            .map(|report| report.probe)
            .unwrap_or("not-applicable"),
        enforcement
            .as_ref()
            .map(|report| report.reason)
            .unwrap_or("non-codex-client"),
        HOOK_PROTOCOL_ID,
    );
    if let Some(report) = enforcement.as_ref()
        && let Some(detail) = report.detail.as_ref()
    {
        println!(
            "|enforcement status={} probe={} reason={} exitSuccess={} deny={} sentinel={} hookEvent={}",
            report.status,
            report.probe,
            report.reason,
            detail.status_success,
            detail.saw_deny,
            detail.saw_sentinel,
            detail.saw_hook_event,
        );
    }
    if client == "codex" && root_hook {
        println!(
            "|codex-app projectConfig={} projectTrust={} hookStateTrust={} reloadHint=restart-open-codex-app-thread-after-install",
            display_path(&project_root, &config_path),
            project_trust,
            hook_state_trust,
        );
    }
    if let Some(status) = trust_status.as_ref()
        && !status.project_trusted
    {
        println!("|trust project=untrusted reason=project-not-trusted");
    }
    if let Some(status) = trust_status.as_ref()
        && !status.missing_events.is_empty()
    {
        println!("|trust missing={}", status.missing_events.join(","));
    }
    if let Some(status) = trust_status.as_ref()
        && !status.stale_events.is_empty()
    {
        println!("|trust stale={}", status.stale_events.join(","));
    }
    for provider in &runtime.providers {
        let runtime_profile = runtime_profiles.providers.iter().find(|profile| {
            profile.manifest_id == provider.manifest_id
                && profile.language_id == provider.language_id
                && profile.provider_id == provider.provider_id
                && profile.binary == provider.binary
        });
        let runtime_profile_status = runtime_profile
            .map(|profile| runtime_profile_status_label(profile.health.status))
            .unwrap_or("missing");
        let resolved_binary = runtime_profile
            .and_then(|profile| profile.resolved_binary.as_deref())
            .unwrap_or("missing");
        println!(
            "|provider language={} provider={} binary={} execution={} runtimeStatus={} resolvedBinary={} roots={} extensions={}",
            provider.language_id,
            provider.provider_id,
            provider.binary,
            provider.execution.as_str(),
            runtime_profile_status,
            resolved_binary,
            provider.source_roots.join(","),
            provider.source_extensions.join(","),
        );
    }
    Ok(())
}

fn run_install(args: &[String]) -> Result<(), String> {
    let mut timings = InstallTimings::new();
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_supported_client(client)?;
    let codex_plugin_scope = codex_plugin_scope_arg(args, client)?;
    let subagent_model =
        subagent_model_arg(client, optional_flag_value(args, "--subagent-model")?)?;
    let project_root = project_root_arg(args)?;
    timings.mark("args");
    let binary_install = ensure_protocol_binary_installed_for_path()?;
    timings.mark("binary");
    remove_legacy_codex_hook_cache_files(&project_root)?;
    timings.mark("legacy-cleanup");
    let activation_path = ensure_project_hook_cache_dir(&project_root)?.join("activation.json");
    let activation_sync = load_or_refresh_default_activation(&activation_path, &project_root)?;
    let activation_status = activation_sync.status;
    let activation = activation_sync.activation;
    timings.mark("activation");
    let runtime_profiles = runtime_profiles_for_activation(&project_root, &activation)?;
    timings.mark("runtime-profiles");
    remove_incompatible_hook_event_state(&project_root)?;
    timings.mark("event-state");
    let client_config_path = default_client_config_path(&project_root.to_string_lossy());
    install_default_client_config(&client_config_path, &activation)?;
    timings.mark("client-config");
    let installed_skill = if client == "claude" {
        Some(install_agent_semantic_protocols_skill(
            &project_root,
            &activation,
            &runtime_profiles,
        )?)
    } else {
        None
    };
    timings.mark("skill");
    let (config_path, extra_config_receipt) = match client {
        "codex" => install_codex_plugin_hooks(&project_root, codex_plugin_scope, &subagent_model)?,
        "claude" => install_claude_project_hooks(&project_root, &subagent_model)?,
        _ => unreachable!("client support checked before install"),
    };
    timings.mark("project-hooks");
    let project_skill_receipt = installed_skill
        .as_ref()
        .and_then(|installed_skill| {
            installed_skill
                .skill_path
                .as_ref()
                .zip(installed_skill.skill_contract_path.as_ref())
        })
        .map(|(skill_path, skill_contract_path)| {
            format!(
                " skill={} skillContract={}",
                display_path(&project_root, skill_path),
                display_path(&project_root, skill_contract_path)
            )
        })
        .unwrap_or_default();
    let plugin_skill_receipt = installed_skill
        .as_ref()
        .and_then(|installed_skill| {
            installed_skill
                .plugin_skill_path
                .as_ref()
                .zip(installed_skill.plugin_skill_contract_path.as_ref())
        })
        .map(|(skill_path, skill_contract_path)| {
            format!(
                " pluginSkill={} pluginSkillContract={}",
                display_path(&project_root, skill_path),
                display_path(&project_root, skill_contract_path)
            )
        })
        .unwrap_or_default();
    println!(
        "[agent-install] client={client} activation={} activationRuntime=derived activationSync={} clientConfig={} config={}{}{}{} binary=asp binaryPath={} binaryInstall={} mode=updated",
        display_path(&project_root, &activation_path),
        activation_status,
        display_path(&project_root, &client_config_path),
        display_path(&project_root, &config_path),
        extra_config_receipt,
        project_skill_receipt,
        plugin_skill_receipt,
        binary_install.path.display(),
        binary_install.status,
    );
    Ok(())
}

struct InstallTimings {
    start: Option<Instant>,
    last: Option<Instant>,
}

impl InstallTimings {
    fn new() -> Self {
        if env::var_os("ASP_HOOK_INSTALL_TIMINGS").is_some() {
            let now = Instant::now();
            Self {
                start: Some(now),
                last: Some(now),
            }
        } else {
            Self {
                start: None,
                last: None,
            }
        }
    }

    fn mark(&mut self, label: &str) {
        let (Some(start), Some(last)) = (self.start, self.last) else {
            return;
        };
        let now = Instant::now();
        eprintln!(
            "[agent-install-timing] step={label} stepMs={:.3} totalMs={:.3}",
            (now - last).as_secs_f64() * 1000.0,
            (now - start).as_secs_f64() * 1000.0,
        );
        self.last = Some(now);
    }
}

fn hook_trigger_prompt_path(project_root: &Path, client: &str) -> PathBuf {
    match client {
        "codex" => project_root
            .join(".codex")
            .join("agent-semantic-protocol")
            .join("hooks")
            .join(HOOK_TRIGGER_PROMPT_FILE_NAME),
        "claude" => project_root
            .join(".claude")
            .join("agent-semantic-protocol")
            .join("hooks")
            .join(HOOK_TRIGGER_PROMPT_FILE_NAME),
        _ => unreachable!("client support checked before prompt path"),
    }
}

fn install_hook_trigger_prompt(project_root: &Path, client: &str) -> Result<PathBuf, String> {
    let path = hook_trigger_prompt_path(project_root, client);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let existing = match fs::read_to_string(&path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };
    let merged = merge_hook_trigger_prompt_document(existing.as_deref());
    fs::write(&path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
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

fn install_claude_project_hooks(
    project_root: &Path,
    subagent_model: &str,
) -> Result<(PathBuf, String), String> {
    let settings_path = default_claude_settings_path(&project_root.to_string_lossy());
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let existing = fs::read_to_string(&settings_path).unwrap_or_default();
    if settings_path.is_file() {
        validate_claude_settings_json(&existing)
            .map_err(|error| format!("refusing to write invalid Claude settings JSON: {error}"))?;
    }
    let merged = merge_claude_settings(&existing, &claude_hook_block(project_root))?;
    validate_claude_settings_json(&merged)
        .map_err(|error| format!("refusing to write invalid Claude settings JSON: {error}"))?;
    fs::write(&settings_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", settings_path.display()))?;
    let trigger_prompt_path = install_hook_trigger_prompt(project_root, "claude")?;
    let subagent_path = install_claude_asp_explorer_agent(project_root, subagent_model)?;
    Ok((
        settings_path,
        format!(
            " triggerPrompt={} subagent={}",
            display_path(project_root, &trigger_prompt_path),
            display_path(project_root, &subagent_path)
        ),
    ))
}

fn install_default_client_config(path: &Path, activation: &HookActivation) -> Result<(), String> {
    let rendered = default_client_config_template_for_source_extensions(
        activation.providers.iter().flat_map(|provider| {
            provider
                .coverage
                .source_extensions
                .iter()
                .map(String::as_str)
        }),
    );
    if path.is_file() {
        let existing = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        load_client_config(path)
            .map(|_| ())
            .map_err(|error| format!("refusing to keep invalid client hook config: {error}"))?;
        if should_refresh_generated_client_config(&existing) && existing != rendered {
            fs::write(path, rendered.as_bytes())
                .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
            load_client_config(path)
                .map(|_| ())
                .map_err(|error| format!("generated invalid client hook config: {error}"))?;
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, rendered.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    load_client_config(path)
        .map(|_| ())
        .map_err(|error| format!("generated invalid client hook config: {error}"))
}

fn should_refresh_generated_client_config(contents: &str) -> bool {
    if !contents.contains("# Semantic agent client hook config.") {
        return false;
    }
    let Ok(config) = toml::from_str::<toml::Value>(contents) else {
        return false;
    };
    let Some(rules) = config.get("rules").and_then(toml::Value::as_array) else {
        return false;
    };
    if rules.len() != 1 {
        return false;
    }
    rules
        .first()
        .and_then(toml::Value::as_table)
        .and_then(|rule| rule.get("id"))
        .and_then(toml::Value::as_str)
        == Some("deny-shell-source-argv")
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

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn decision_kind_label(kind: DecisionKind) -> &'static str {
    match kind {
        DecisionKind::Allow => "allow",
        DecisionKind::Block => "block",
        DecisionKind::Deny => "deny",
    }
}

fn reason_kind_label(kind: ReasonKind) -> &'static str {
    match kind {
        ReasonKind::None => "none",
        ReasonKind::DirectSourceRead => "direct-source-read",
        ReasonKind::BulkSourceDump => "bulk-source-dump",
        ReasonKind::RawBroadSearch => "raw-broad-search",
        ReasonKind::SourceDirectoryEnumeration => "source-directory-enumeration",
        ReasonKind::AgentSearchJson => "agent-search-json",
        ReasonKind::SemanticAstPatchRequired => "semantic-ast-patch-required",
        ReasonKind::SubagentReceiptRequired => "subagent-receipt-required",
    }
}

fn runtime_profile_status_label(status: RuntimeProviderHealthStatus) -> &'static str {
    match status {
        RuntimeProviderHealthStatus::Available => "available",
        RuntimeProviderHealthStatus::Missing => "missing",
        RuntimeProviderHealthStatus::Unexecutable => "unexecutable",
    }
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn optional_flag_value<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>, String> {
    let inline_prefix = format!("{flag}=");
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix(&inline_prefix) {
            return Ok(Some(value));
        }
        if arg == flag {
            let value = args
                .get(index + 1)
                .ok_or_else(|| format!("{flag} requires a value"))?;
            if value.starts_with("--") {
                return Err(format!("{flag} requires a value"));
            }
            return Ok(Some(value));
        }
    }
    Ok(None)
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
