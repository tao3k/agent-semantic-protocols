//! CLI entrypoint for installing and replaying `semantic-agent-hook` activations.

use crate::activation_store::{
    default_activation_path, load_activation, load_or_sync_activation, write_activation,
};
use crate::codex_config::{
    ROOT_BLOCK_BEGIN, ROOT_BLOCK_END, codex_hook_block, codex_user_trust_state_status,
    install_codex_user_trust_state, merge_codex_config, validate_codex_config_toml,
};
use crate::event_state::append_hook_event_state;
use crate::provider_manifest::{build_default_activation, provider_binary_available};
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
    run()
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
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
        .unwrap_or_else(|| default_activation_path(&PathBuf::from(".")));
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
