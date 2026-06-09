//! Runtime for the `asp hook` command surface.

#[path = "hook_runtime_skill.rs"]
mod hook_runtime_skill;

use super::{
    codex_enforcement_report, ensure_protocol_binary_installed_for_path, protocol_binary_on_path,
};
use agent_semantic_hook::{
    ActiveContextRecord, DecisionKind, DecisionSubject, HOOK_DECISION_SCHEMA_ID,
    HOOK_DECISION_SCHEMA_VERSION, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION,
    HookClassificationRequest, HookDecision, ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, ReasonKind,
    RuntimeProviderHealthStatus, append_hook_event_state, apply_repeated_deny_replay,
    build_default_activation, classify_hook_with_config, claude_hook_block, codex_hook_block,
    codex_user_trust_state_status, default_activation_path, default_claude_settings_path,
    default_client_config_path, default_client_config_template, discover_activation_path,
    install_codex_user_trust_state, load_activation, load_client_config, load_or_sync_activation,
    merge_claude_settings, merge_codex_config, parse_payload, record_active_context,
    remove_incompatible_hook_event_state, remove_legacy_codex_hook_cache_files,
    render_platform_response, runtime_profiles_for_activation, runtime_profiles_for_runtime,
    validate_claude_settings_json, validate_codex_config_toml, write_activation,
};
use agent_semantic_runtime::ensure_project_hook_cache_dir;
use hook_runtime_skill::install_agent_semantic_protocols_skill;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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
    let mut stdin = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut stdin) {
        emit_hook_runtime_failure(
            client,
            event,
            emit,
            &format!("failed to read hook payload from stdin: {error}"),
        )?;
        return Ok(());
    }
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
    annotate_payload_context(&mut decision, &payload);
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

fn annotate_payload_context(decision: &mut HookDecision, payload: &serde_json::Value) {
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
    let client_config_status = if client_config_path.is_file() {
        load_client_config(&client_config_path).map_err(|error| {
            format!(
                "invalid client hook config {}: {error}",
                display_path(&project_root, &client_config_path)
            )
        })?;
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
    let trust_status = if client == "codex" {
        codex_user_trust_state_status(&config_path).ok()
    } else {
        None
    };
    let trust = trust_status.as_ref().is_some_and(|status| status.trusted);
    let trust_config = trust_status
        .as_ref()
        .map(|status| status.trust_config_path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "[agent-doctor] status=ok client={client} providers={} activation={} activationRuntime=derived config={} clientConfig={} clientConfigStatus={} hook={} trust={} trustConfig={} binary={} binaryPath={} enforcement={} enforcementProbe={} enforcementReason={} protocol={}",
        runtime.providers.len(),
        display_path(&project_root, &activation_path),
        config_path.is_file(),
        display_path(&project_root, &client_config_path),
        client_config_status,
        root_hook,
        trust,
        trust_config,
        hook_binary,
        hook_binary_path,
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
    if let Some(status) = trust_status.as_ref()
        && !status.missing_events.is_empty()
    {
        println!("|trust missing={}", status.missing_events.join(","));
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
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_supported_client(client)?;
    let project_root = project_root_arg(args)?;
    let binary_install = ensure_protocol_binary_installed_for_path()?;
    remove_legacy_codex_hook_cache_files(&project_root)?;
    let activation_path = ensure_project_hook_cache_dir(&project_root)?.join("activation.json");
    let activation = build_default_activation(&project_root)?;
    write_activation(&activation_path, &activation)?;
    let runtime_profiles = runtime_profiles_for_activation(&project_root, &activation)?;
    remove_incompatible_hook_event_state(&project_root)?;
    let client_config_path = default_client_config_path(&project_root.to_string_lossy());
    install_default_client_config(&client_config_path)?;
    let skill_path =
        install_agent_semantic_protocols_skill(&project_root, &activation, &runtime_profiles)?;
    let (config_path, extra_config_receipt) = match client {
        "codex" => install_codex_project_hooks(&project_root)?,
        "claude" => install_claude_project_hooks(&project_root)?,
        _ => unreachable!("client support checked before install"),
    };
    println!(
        "[agent-install] client={client} activation={} activationRuntime=derived clientConfig={} config={}{} skill={} binary=asp binaryPath={} binaryInstall={} mode=updated",
        display_path(&project_root, &activation_path),
        display_path(&project_root, &client_config_path),
        display_path(&project_root, &config_path),
        extra_config_receipt,
        display_path(&project_root, &skill_path),
        binary_install.path.display(),
        binary_install.status,
    );
    Ok(())
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

fn install_codex_project_hooks(project_root: &Path) -> Result<(PathBuf, String), String> {
    let codex_dir = project_root.join(".codex");
    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.is_file() {
        validate_codex_config_toml(&existing)
            .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    }
    let merged = merge_codex_config(&existing, &codex_hook_block(project_root));
    validate_codex_config_toml(&merged)
        .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    fs::write(&config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    let user_config_path = install_codex_user_trust_state(&config_path)?;
    Ok((
        config_path,
        format!(" trustConfig={}", user_config_path.display()),
    ))
}

fn install_claude_project_hooks(project_root: &Path) -> Result<(PathBuf, String), String> {
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
    Ok((settings_path, String::new()))
}

fn install_default_client_config(path: &Path) -> Result<(), String> {
    if path.is_file() {
        load_client_config(path)
            .map(|_| ())
            .map_err(|error| format!("refusing to keep invalid client hook config: {error}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, default_client_config_template())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    load_client_config(path)
        .map(|_| ())
        .map_err(|error| format!("generated invalid client hook config: {error}"))
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
            "--client" | "--activation" | "--config" | "--emit" | "--output"
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
