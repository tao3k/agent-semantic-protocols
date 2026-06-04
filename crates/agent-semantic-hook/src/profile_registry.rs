//! Provider profile registry writer for `agent-semantic-protocol` hook caches.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::protocol::{CommandTemplate, HOOK_PROTOCOL_ID, HOOK_PROTOCOL_VERSION, HookRoutes};
use crate::protocol_activation::{ActivatedProviderConfig, HookActivation, ProviderManifest};
use crate::provider_manifest::provider_manifests;

/// Write the cache-local provider profile registry for an activation.
pub fn write_profile_registry(
    profiles_dir: &Path,
    activation: &HookActivation,
) -> Result<PathBuf, String> {
    fs::create_dir_all(profiles_dir)
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
        "schemaId": "agent.semantic-protocols.hook.profile-registry",
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
    remove_legacy_profile_shards(profiles_dir)?;
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

/// Remove legacy hook cache files from previous `semantic-agent` layouts.
pub fn remove_legacy_codex_hook_cache_files(project_root: &Path) -> Result<(), String> {
    let legacy_codex_dir = project_root.join(".codex").join("agent-semantic-hook");
    remove_legacy_hook_dir(&legacy_codex_dir)?;
    let legacy_default_cache_dir = project_root.join(".cache").join("agent-semantic-hook");
    remove_legacy_hook_dir(&legacy_default_cache_dir)?;
    let legacy_semantic_protocol_cache_dir =
        project_root.join(".cache").join("semantic-agent-protocol");
    remove_legacy_hook_dir(&legacy_semantic_protocol_cache_dir.join("hooks"))?;
    remove_empty_dir(&legacy_semantic_protocol_cache_dir)?;
    if let Some(cache_root) = env::var_os("PRJ_HOME_CACHE").filter(|value| !value.is_empty()) {
        let cache_root = PathBuf::from(cache_root);
        remove_legacy_hook_dir(&cache_root.join("agent-semantic-hook"))?;
        let legacy_semantic_protocol_cache_dir = cache_root.join("semantic-agent-protocol");
        remove_legacy_hook_dir(&legacy_semantic_protocol_cache_dir.join("hooks"))?;
        remove_empty_dir(&legacy_semantic_protocol_cache_dir)?;
    }
    Ok(())
}

fn remove_legacy_hook_dir(legacy_dir: &Path) -> Result<(), String> {
    if !legacy_dir.is_dir() {
        return Ok(());
    }
    for name in ["activation.json", "events.jsonl", "profiles.json"] {
        let path = legacy_dir.join(name);
        if path.is_file() {
            fs::remove_file(&path)
                .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        }
    }
    remove_legacy_profile_shards(legacy_dir)?;
    remove_empty_dir(legacy_dir)?;
    Ok(())
}

fn remove_empty_dir(path: &Path) -> Result<(), String> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => Ok(()),
        Err(error) => Err(format!("failed to remove {}: {error}", path.display())),
    }
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
