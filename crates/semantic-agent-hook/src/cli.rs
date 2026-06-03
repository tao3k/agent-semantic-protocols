//! CLI entrypoint for installing and replaying `semantic-agent-hook` activations.

use crate::activation_store::{
    default_activation_path, discover_activation_path, load_activation, load_or_sync_activation,
    write_activation,
};
use crate::codex_config::{
    ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, codex_hook_block, codex_user_trust_state_status,
    install_codex_user_trust_state, merge_codex_config, validate_codex_config_toml,
};
use crate::event_state::append_hook_event_state;
use crate::protocol::{CommandTemplate, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookRoutes};
use crate::protocol_activation::{ActivatedProviderConfig, HookActivation, ProviderManifest};
use crate::provider_manifest::{
    build_default_activation, provider_binary_available, provider_manifests,
};
use crate::{
    DecisionKind, DecisionSubject, HookDecision, ReasonKind, classify_hook, parse_payload,
    render_platform_response,
};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const AGENT_SEMANTIC_PROTOCOLS_SKILL_MD: &str = include_str!("../../../SKILL.md");

/// Run the `semantic-agent-hook` CLI using process arguments and standard IO.
pub fn run_cli_from_env() -> Result<(), String> {
    run_cli_args(env::args().skip(1))
}

/// Run the `semantic-agent-hook` CLI using caller-provided arguments.
pub fn run_cli_args<I, S>(args: I) -> Result<(), String>
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
        _ => Err(
            "usage: semantic-agent-hook <install|doctor|hook> --client codex [PROJECT_ROOT]"
                .to_string(),
        ),
    }
}

fn run_hook(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client")
        .ok_or_else(|| "missing required --client <client>".to_string())?;
    let emit = flag_value(args, "--emit").unwrap_or("platform");
    let event = first_positional(args).ok_or_else(|| "missing hook event".to_string())?;
    let activation_path = flag_value(args, "--activation")
        .map(PathBuf::from)
        .unwrap_or_else(default_or_discovered_activation_path);
    let runtime = match load_activation(&activation_path) {
        Ok(registry) => registry,
        Err(error) => {
            emit_activation_load_failure(client, event, emit, &activation_path, &error)?;
            return Ok(());
        }
    };
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
    let decision = classify_hook(&runtime, client, event, &payload);
    crate::dev_context::record_active_context(&activation_path, client, event, &payload, &decision);
    if let Err(error) = append_hook_event_state(&activation_path, &decision) {
        eprintln!("[semantic-agent-hook] failed to update hook state: {error}");
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

fn emit_activation_load_failure(
    client: &str,
    event: &str,
    emit: &str,
    activation_path: &Path,
    error: &str,
) -> Result<(), String> {
    eprintln!(
        "[semantic-agent-hook] activation disabled for this hook event: {}: {error}",
        activation_path.display()
    );
    emit_hook_runtime_failure(
        client,
        event,
        emit,
        &format!(
            "Semantic hook activation could not be loaded; allowing tool use so activation can be repaired: {error}"
        ),
    )
}

fn emit_hook_runtime_failure(
    client: &str,
    event: &str,
    emit: &str,
    message: &str,
) -> Result<(), String> {
    let decision = HookDecision {
        schema_id: crate::HOOK_DECISION_SCHEMA_ID,
        schema_version: crate::HOOK_DECISION_SCHEMA_VERSION,
        protocol_id: crate::HOOK_PROTOCOL_ID,
        protocol_version: crate::HOOK_PROTOCOL_VERSION,
        platform: client.to_string(),
        event: event.to_string(),
        decision: DecisionKind::Allow,
        reason_kind: ReasonKind::None,
        language_ids: Vec::new(),
        subject: DecisionSubject::default(),
        routes: Vec::new(),
        message: message.to_string(),
    };
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

fn run_doctor(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_codex_client(client)?;
    let project_root = project_root_arg(args);
    let activation_path = flag_value(args, "--activation")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_activation_path(&project_root));
    let runtime = load_or_sync_activation(&activation_path, &project_root)?;
    let config_path = project_root.join(".codex").join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap_or_default();
    let root_hook = config.contains(ROOT_BLOCK_BEGIN) && config.contains(ROOT_BLOCK_END);
    let hook_binary = provider_binary_available("semantic-agent-hook");
    let trust_status = codex_user_trust_state_status(&config_path).ok();
    let trust = trust_status.as_ref().is_some_and(|status| status.trusted);
    let trust_config = trust_status
        .as_ref()
        .map(|status| status.trust_config_path.display().to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    println!(
        "[agent-doctor] status=ok client={client} providers={} activation={} config={} hook={} trust={} trustConfig={} binary={} protocol={}",
        runtime.providers.len(),
        display_path(&project_root, &activation_path),
        config_path.is_file(),
        root_hook,
        trust,
        trust_config,
        hook_binary,
        crate::HOOK_PROTOCOL_ID,
    );
    if let Some(status) = trust_status.as_ref()
        && !status.missing_events.is_empty()
    {
        println!("|trust missing={}", status.missing_events.join(","));
    }
    for provider in runtime.providers {
        println!(
            "|provider language={} provider={} binary={} roots={} extensions={}",
            provider.language_id,
            provider.provider_id,
            provider.binary,
            provider.source_roots.join(","),
            provider.source_extensions.join(","),
        );
    }
    Ok(())
}

fn run_install(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    ensure_codex_client(client)?;
    let project_root = project_root_arg(args);
    let codex_dir = project_root.join(".codex");
    let activation_path = default_activation_path(&project_root);
    if let Some(parent) = activation_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let activation = build_default_activation(&project_root)?;
    write_activation(&activation_path, &activation)?;
    write_profile_registry(&project_root, &activation)?;
    let skill_path = install_agent_semantic_protocols_skill(&project_root)?;

    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.is_file() {
        validate_codex_config_toml(&existing)
            .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    }
    let merged = merge_codex_config(&existing, &codex_hook_block());
    validate_codex_config_toml(&merged)
        .map_err(|error| format!("refusing to write invalid Codex config TOML: {error}"))?;
    fs::write(&config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    let user_config_path = install_codex_user_trust_state(&config_path)?;

    println!(
        "[agent-install] client={client} activation={} config={} trustConfig={} skill={} binary=semantic-agent-hook mode=updated",
        display_path(&project_root, &activation_path),
        display_path(&project_root, &config_path),
        user_config_path.display(),
        display_path(&project_root, &skill_path),
    );
    Ok(())
}

fn write_profile_registry(
    project_root: &Path,
    activation: &HookActivation,
) -> Result<PathBuf, String> {
    let profiles_dir = project_root.join(".codex").join("semantic-agent-hook");
    fs::create_dir_all(&profiles_dir)
        .map_err(|error| format!("failed to create {}: {error}", profiles_dir.display()))?;

    let manifests = provider_manifests();
    let mut profiles = Vec::new();
    for activated in &activation.providers {
        let manifest = manifests
            .iter()
            .find(|manifest| manifest.manifest_id == activated.manifest_id)
            .ok_or_else(|| {
                format!(
                    "missing provider manifest for activated provider {}",
                    activated.manifest_id
                )
            })?;
        profiles.push(profile_entry(activated, manifest));
    }

    let registry = serde_json::json!({
        "schemaId": "agent.semantic-protocols.semantic-agent-hook-profile-registry",
        "schemaVersion": "1",
        "protocolId": HOOK_PROTOCOL_ID,
        "protocolVersion": HOOK_PROTOCOL_VERSION,
        "projectRoot": ".",
        "profiles": profiles,
    });
    let output = serde_json::to_string_pretty(&registry)
        .map_err(|error| format!("failed to serialize provider profiles: {error}"))?;
    let profile_path = profiles_dir.join("profiles.json");
    fs::write(&profile_path, format!("{output}\n").as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", profile_path.display()))?;
    remove_legacy_profile_shards(&profiles_dir)?;
    Ok(profile_path)
}

fn profile_entry(
    activated: &ActivatedProviderConfig,
    manifest: &ProviderManifest,
) -> serde_json::Value {
    serde_json::json!({
        "languageId": activated.language_id,
        "providerId": activated.provider_id,
        "binary": activated.binary,
        "namespace": manifest.namespace,
        "sourceExtensions": activated.coverage.source_extensions,
        "configFiles": activated.coverage.config_files,
        "sourceRoots": activated.coverage.source_roots,
        "ignoredPathPrefixes": activated.coverage.ignored_path_prefixes,
        "policy": manifest.policy,
        "commands": profile_commands(&manifest.routes, &activated.binary, &activated.provider_command_prefix),
    })
}

fn profile_commands(
    routes: &HookRoutes,
    binary: &str,
    provider_command_prefix: &[String],
) -> serde_json::Value {
    let mut commands = serde_json::Map::new();
    commands.insert(
        "prime".to_string(),
        profile_command(&routes.prime, binary, provider_command_prefix),
    );
    commands.insert(
        "owner".to_string(),
        profile_command(&routes.owner, binary, provider_command_prefix),
    );
    commands.insert(
        "fzf".to_string(),
        profile_command(&routes.fzf, binary, provider_command_prefix),
    );
    if let Some(query) = &routes.query {
        commands.insert(
            "query".to_string(),
            profile_command(query, binary, provider_command_prefix),
        );
    }
    commands.insert(
        "ingest".to_string(),
        profile_command(&routes.ingest, binary, provider_command_prefix),
    );
    commands.insert(
        "checkChanged".to_string(),
        profile_command(&routes.check_changed, binary, provider_command_prefix),
    );
    if let Some(guide) = &routes.guide {
        commands.insert(
            "guide".to_string(),
            profile_command(guide, binary, provider_command_prefix),
        );
    }
    serde_json::Value::Object(commands)
}

fn profile_command(
    command: &CommandTemplate,
    binary: &str,
    provider_command_prefix: &[String],
) -> serde_json::Value {
    let argv = profile_command_argv(command, binary, provider_command_prefix);
    let mut value = serde_json::Map::new();
    value.insert(
        "text".to_string(),
        serde_json::Value::String(argv.join(" ")),
    );
    value.insert("argv".to_string(), serde_json::json!(argv));
    if let Some(stdin_mode) = command.stdin_mode {
        value.insert("stdinMode".to_string(), serde_json::json!(stdin_mode));
    }
    serde_json::Value::Object(value)
}

fn profile_command_argv(
    command: &CommandTemplate,
    binary: &str,
    provider_command_prefix: &[String],
) -> Vec<String> {
    let mut argv = if !provider_command_prefix.is_empty()
        && command
            .argv
            .first()
            .is_some_and(|command| command == binary)
    {
        provider_command_prefix
            .iter()
            .cloned()
            .chain(command.argv.iter().skip(1).cloned())
            .collect()
    } else {
        command.argv.clone()
    };
    for argument in &mut argv {
        if argument == "{projectRoot}" {
            *argument = ".".to_string();
        }
    }
    argv
}

fn remove_legacy_profile_shards(profiles_dir: &Path) -> Result<(), String> {
    for entry in fs::read_dir(profiles_dir)
        .map_err(|error| format!("failed to read {}: {error}", profiles_dir.display()))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read profile registry entry: {error}"))?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name != "profiles.json"
            && file_name.starts_with("profiles.")
            && file_name.ends_with(".json")
        {
            fs::remove_file(entry.path())
                .map_err(|error| format!("failed to remove {}: {error}", entry.path().display()))?;
        }
    }
    Ok(())
}

fn default_agent_skill_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".agents")
        .join("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.md")
}

fn install_agent_semantic_protocols_skill(project_root: &Path) -> Result<PathBuf, String> {
    let skill_path = default_agent_skill_path(project_root);
    if let Some(parent) = skill_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(
        &skill_path,
        format!("{}\n", AGENT_SEMANTIC_PROTOCOLS_SKILL_MD.trim_end()),
    )
    .map_err(|error| format!("failed to write {}: {error}", skill_path.display()))?;
    Ok(skill_path)
}

fn project_root_arg(args: &[String]) -> PathBuf {
    positionals(args)
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn ensure_codex_client(client: &str) -> Result<(), String> {
    if client == "codex" {
        Ok(())
    } else {
        Err(format!("unsupported --client {client}; expected codex"))
    }
}

fn display_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
            "--client" | "--activation" | "--emit" | "--output"
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
