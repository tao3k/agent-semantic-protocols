//! Installation owner for hook runtime and Codex plugin surfaces.

use super::hook_runtime_codex_plugin::{
    CodexPluginScope, codex_plugin_scope_arg, codex_project_plugin_cache_skill_path,
    install_codex_plugin_hooks, sync_codex_project_plugin_cache,
};
use super::hook_runtime_skill::{
    PluginSkillScope, install_agent_semantic_protocols_agent_config,
    install_agent_semantic_protocols_plugin_skill, install_agent_semantic_protocols_skill,
};
use super::hook_runtime_subagent::{install_claude_resident_agents, subagent_model_arg};
use super::{
    display_path, ensure_supported_client, flag_value, optional_flag_value, project_root_arg,
};
use crate::command::{ensure_protocol_binary_installed_for_path, run_org_state_sync};
use agent_semantic_hook::{
    claude_hook_block, default_claude_settings_path, default_client_config_path,
    load_client_config, load_or_refresh_default_activation, merge_claude_settings,
    remove_incompatible_hook_event_state, runtime_profiles_for_activation,
    validate_claude_settings_json,
};
use agent_semantic_runtime::{project_activation_path, project_runtime_state};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(super) fn run_install(args: &[String]) -> Result<(), String> {
    let client = flag_value(args, "--client").unwrap_or("codex");
    if client == "codex" {
        return Err(
            "Codex plugin installation uses `asp install plugin --codex [PROJECT_ROOT]`; direct hook configuration is not a Codex surface."
                .to_string(),
        );
    }
    run_install_for_client(client, args, "agent-install")
}

pub(in crate::command) fn run_codex_plugin_install_args(args: &[String]) -> Result<(), String> {
    if optional_flag_value(args, "--client")?.is_some() {
        return Err(
            "asp install plugin --codex does not accept --client; use `asp install plugin --codex [PROJECT_ROOT]`"
                .to_string(),
        );
    }
    let mut global_args = args.to_vec();
    if !global_args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--global" | "--global-plugin"))
    {
        global_args.push("--global".to_string());
    }
    run_install_for_client("codex", &global_args, "plugin-install")
}

fn run_install_for_client(
    client: &str,
    args: &[String],
    receipt_label: &str,
) -> Result<(), String> {
    let mut timings = InstallTimings::new();
    ensure_supported_client(client)?;
    let codex_plugin_scope = codex_plugin_scope_arg(args, client)?;
    let subagent_model =
        subagent_model_arg(client, optional_flag_value(args, "--subagent-model")?)?;
    let project_root = project_root_arg(args)?;
    timings.mark("args");
    let runtime_state = project_runtime_state(&project_root)?;
    timings.mark("runtime-state");
    let org_state_sync = run_org_state_sync(&project_root)?;
    timings.mark("org-state");
    let binary_install = ensure_protocol_binary_installed_for_path()?;
    timings.mark("binary");
    let activation_path = project_activation_path(&project_root)?;
    let activation_sync = load_or_refresh_default_activation(&activation_path, &project_root)?;
    let activation_status = activation_sync.status;
    let activation = activation_sync.activation;
    timings.mark("activation");
    let runtime_profiles = runtime_profiles_for_activation(&project_root, &activation)?;
    timings.mark("runtime-profiles");
    remove_incompatible_hook_event_state(&project_root)?;
    timings.mark("event-state");
    let client_config_path = default_client_config_path(&project_root.to_string_lossy());
    let user_config_status = reconcile_managed_hook_client_config(&client_config_path)?;
    timings.mark("user-config");
    let (config_path, extra_config_receipt) = match client {
        "codex" => install_codex_plugin_hooks(&project_root, codex_plugin_scope, &subagent_model)?,
        "claude" => install_claude_project_hooks(&project_root, &subagent_model)?,
        _ => unreachable!("client support checked before install"),
    };
    timings.mark("project-hooks");
    let agent_config_path = install_agent_semantic_protocols_agent_config(&project_root)?;
    timings.mark("agent-config");
    let installed_skill = Some(match client {
        "codex" => install_agent_semantic_protocols_plugin_skill(
            &project_root,
            match codex_plugin_scope {
                CodexPluginScope::Project => PluginSkillScope::Project,
                CodexPluginScope::Global => PluginSkillScope::Global,
            },
            &activation,
            &runtime_profiles,
        )?,
        "claude" => {
            install_agent_semantic_protocols_skill(&project_root, &activation, &runtime_profiles)?
        }
        _ => unreachable!("client support checked before install"),
    });
    timings.mark("skill");
    let plugin_cache_path =
        if client == "codex" && matches!(codex_plugin_scope, CodexPluginScope::Project) {
            sync_codex_project_plugin_cache(&project_root)?
        } else {
            None
        };
    if client == "codex" && matches!(codex_plugin_scope, CodexPluginScope::Global) {
        let legacy_project_cache = project_root.join(".codex/plugins/cache/asp-project");
        if legacy_project_cache.exists() {
            fs::remove_dir_all(&legacy_project_cache).map_err(|error| {
                format!(
                    "failed to remove legacy Codex project plugin cache {}: {error}",
                    legacy_project_cache.display()
                )
            })?;
        }
    }
    timings.mark("plugin-cache");
    let project_skill_receipt = installed_skill
        .as_ref()
        .and_then(|installed_skill| installed_skill.skill_path.as_ref())
        .map(|skill_path| format!(" skill={}", display_path(&project_root, skill_path)))
        .unwrap_or_default();
    let plugin_skill_path =
        if client == "codex" && matches!(codex_plugin_scope, CodexPluginScope::Project) {
            Some(codex_project_plugin_cache_skill_path(&project_root)?)
        } else {
            installed_skill
                .as_ref()
                .and_then(|installed_skill| installed_skill.plugin_skill_path.clone())
        };
    let plugin_skill_receipt = plugin_skill_path
        .as_ref()
        .map(|skill_path| format!(" pluginSkill={}", display_path(&project_root, skill_path)))
        .unwrap_or_default();
    let plugin_cache_receipt = plugin_cache_path
        .as_ref()
        .map(|cache_path| format!(" pluginCache={}", display_path(&project_root, cache_path)))
        .unwrap_or_default();
    let user_config_receipt = format!(
        " userConfig={} userConfigStatus={}",
        display_path(&project_root, &client_config_path),
        user_config_status.as_str()
    );
    let binary_paths = binary_install
        .paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "[{receipt_label}] client={client} activation={} activationRuntime=derived activationSync={}{} agentConfig={} orgState={} orgStateSync={} orgSourceIndex={} config={}{}{}{}{} binary=asp binaryPath={} binaryPaths={} binaryInstall={} binaryArtifactDigest={} binarySwitch=atomic mode=updated",
        display_path(&project_root, &activation_path),
        activation_status,
        user_config_receipt,
        display_path(&project_root, &agent_config_path),
        display_path(&project_root, &runtime_state.protocol_home.join("org")),
        org_state_sync.status,
        org_state_sync.source_index_status,
        display_path(&project_root, &config_path),
        extra_config_receipt,
        project_skill_receipt,
        plugin_skill_receipt,
        plugin_cache_receipt,
        binary_install.path.display(),
        binary_paths,
        binary_install.status,
        binary_install.artifact_digest,
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
    let subagent_path = install_claude_resident_agents(project_root, subagent_model)?;
    Ok((
        settings_path,
        format!(" subagent={}", display_path(project_root, &subagent_path)),
    ))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UserConfigStatus {
    Current,
    Created,
    MigratedManaged,
}

impl UserConfigStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Created => "created",
            Self::MigratedManaged => "migrated-managed",
        }
    }
}

fn reconcile_managed_hook_client_config(
    path: &std::path::Path,
) -> Result<UserConfigStatus, String> {
    let current_template = agent_semantic_hook::default_client_config_template();
    let current_fingerprint = config_contract_fingerprint(&current_template)?;
    let current_bytes = current_template.as_bytes();
    let sidecar_path = managed_config_digest_path(path);

    if !path.exists() {
        write_managed_config_pair(path, current_bytes, &sidecar_path)?;
        verify_current_managed_config(path, &current_fingerprint, current_bytes, &sidecar_path)?;
        return Ok(UserConfigStatus::Created);
    }

    let existing_bytes = std::fs::read(path).map_err(|error| {
        format!(
            "failed to read user hook config {}: {error}",
            path.display()
        )
    })?;
    let existing_config =
        load_client_config(path).map_err(|error| format!("invalid user hook config: {error}"))?;
    if existing_config.contract_fingerprint() == Some(current_fingerprint.as_str()) {
        if existing_bytes == current_bytes && !sidecar_path.exists() {
            atomic_write(
                &sidecar_path,
                managed_config_digest(&existing_bytes).as_bytes(),
            )?;
        }
        return Ok(UserConfigStatus::Current);
    }

    let sidecar_matches = std::fs::read_to_string(&sidecar_path)
        .ok()
        .is_some_and(|digest| digest.trim() == managed_config_digest(&existing_bytes));
    let recognized_legacy_default = existing_config.contract_fingerprint().is_none()
        && same_recognized_legacy_managed_config(&existing_bytes, current_bytes)?;
    if sidecar_matches || recognized_legacy_default {
        write_managed_config_pair(path, current_bytes, &sidecar_path)?;
        verify_current_managed_config(path, &current_fingerprint, current_bytes, &sidecar_path)?;
        return Ok(UserConfigStatus::MigratedManaged);
    }

    Err(format!(
        "user-config-contract-unproven: refusing to overwrite {}",
        path.display()
    ))
}

fn managed_config_digest_path(config_path: &std::path::Path) -> std::path::PathBuf {
    let file_name = config_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    config_path.with_file_name(format!("{file_name}.managed.sha256"))
}

fn config_contract_fingerprint(template: &str) -> Result<String, String> {
    let value = toml::from_str::<toml::Value>(template)
        .map_err(|error| format!("invalid default hook config template: {error}"))?;
    value
        .get("contractFingerprint")
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "default hook config template is missing contractFingerprint".to_string())
}

fn same_recognized_legacy_managed_config(existing: &[u8], current: &[u8]) -> Result<bool, String> {
    let existing_text = std::str::from_utf8(existing)
        .map_err(|error| format!("invalid utf8 user hook config: {error}"))?;
    let existing = toml::from_str::<toml::Value>(existing_text)
        .map_err(|error| format!("invalid user hook config: {error}"))?;
    let current_text = std::str::from_utf8(current)
        .map_err(|error| format!("invalid utf8 default hook config template: {error}"))?;
    let current = toml::from_str::<toml::Value>(current_text)
        .map_err(|error| format!("invalid default hook config template: {error}"))?;
    let current_without_fingerprint = remove_top_level_contract_fingerprint(current)?;
    let current_pre_rules = remove_top_level_rules(current_without_fingerprint.clone())?;
    Ok(existing == current_without_fingerprint
        || existing == current_pre_rules
        || same_audited_pre_rules_prompt_legacy(&existing, &current_pre_rules)?)
}

fn same_audited_pre_rules_prompt_legacy(
    existing: &toml::Value,
    current_pre_rules: &toml::Value,
) -> Result<bool, String> {
    const SIGNATURES: &[(&[&str], usize, &str)] = &[
        (&["agentSessionGuide", "register"], 873, "eb75c832f874"),
        (&["agentSessionGuide", "status"], 330, "f629b91cbd7b"),
        (
            &["agentSessionMessages", "sourceAccessCompact"],
            539,
            "ad46a65d6d4e",
        ),
        (
            &["agentSessionMessages", "sourceAccessCompactRepeated"],
            326,
            "fb79b457af27",
        ),
        (
            &["agentSessionMessages", "sourceAccessCompactSubagent"],
            340,
            "3338fe7386c9",
        ),
    ];
    if !SIGNATURES.iter().all(|(path, length, digest_prefix)| {
        toml_string_at_path(existing, path).is_some_and(|value| {
            value.len() == *length
                && managed_config_digest(value.as_bytes()).starts_with(digest_prefix)
        })
    }) {
        return Ok(false);
    }

    let mut normalized_existing = existing.clone();
    let mut normalized_current = current_pre_rules.clone();
    for path in SIGNATURES.iter().map(|(path, _, _)| *path) {
        remove_toml_path(&mut normalized_existing, path)?;
    }
    for path in [
        &["agentSessionGuide", "register"][..],
        &["agentSessionGuide", "status"][..],
    ] {
        remove_toml_path(&mut normalized_current, path)?;
    }
    Ok(normalized_existing == normalized_current)
}

fn toml_string_at_path<'a>(value: &'a toml::Value, path: &[&str]) -> Option<&'a str> {
    path.iter()
        .try_fold(value, |current, key| current.get(*key))?
        .as_str()
}

fn remove_toml_path(value: &mut toml::Value, path: &[&str]) -> Result<(), String> {
    let Some((key, parents)) = path.split_last() else {
        return Err("managed config normalization path is empty".to_string());
    };
    let parent = parents.iter().try_fold(value, |current, parent| {
        current
            .get_mut(*parent)
            .ok_or_else(|| format!("managed config normalization path is missing {parent}"))
    })?;
    parent
        .as_table_mut()
        .ok_or_else(|| format!("managed config normalization parent is not a table: {parents:?}"))?
        .remove(*key);
    Ok(())
}

fn remove_top_level_contract_fingerprint(mut config: toml::Value) -> Result<toml::Value, String> {
    config
        .as_table_mut()
        .ok_or_else(|| "hook config TOML root must be a table".to_string())?
        .remove("contractFingerprint");
    Ok(config)
}

fn remove_top_level_rules(mut config: toml::Value) -> Result<toml::Value, String> {
    config
        .as_table_mut()
        .ok_or_else(|| "hook config TOML root must be a table".to_string())?
        .remove("rules");
    Ok(config)
}

fn write_managed_config_pair(
    config_path: &std::path::Path,
    config_bytes: &[u8],
    sidecar_path: &std::path::Path,
) -> Result<(), String> {
    atomic_write(config_path, config_bytes)?;
    atomic_write(sidecar_path, managed_config_digest(config_bytes).as_bytes())
}

fn verify_current_managed_config(
    config_path: &std::path::Path,
    current_fingerprint: &str,
    current_bytes: &[u8],
    sidecar_path: &std::path::Path,
) -> Result<(), String> {
    let loaded = load_client_config(config_path)
        .map_err(|error| format!("failed to reload managed hook config: {error}"))?;
    if loaded.contract_fingerprint() != Some(current_fingerprint) {
        return Err("managed hook config fingerprint verification failed".to_string());
    }
    let written = std::fs::read(config_path).map_err(|error| {
        format!(
            "failed to reread managed hook config {}: {error}",
            config_path.display()
        )
    })?;
    if written != current_bytes {
        return Err("managed hook config byte verification failed".to_string());
    }
    let sidecar = std::fs::read_to_string(sidecar_path).map_err(|error| {
        format!(
            "failed to reread managed config sidecar {}: {error}",
            sidecar_path.display()
        )
    })?;
    if sidecar.trim() != managed_config_digest(current_bytes) {
        return Err("managed hook config sidecar verification failed".to_string());
    }
    Ok(())
}

fn managed_config_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;

    let parent = path
        .parent()
        .ok_or_else(|| format!("managed config path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create managed config directory {}: {error}",
            parent.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let nonce = format!(
        "{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    );
    let temporary = parent.join(format!(".{file_name}.{nonce}.tmp"));
    let write_result = (|| -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "failed to create managed config temporary {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(bytes).map_err(|error| {
            format!(
                "failed to write managed config temporary {}: {error}",
                temporary.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync managed config temporary {}: {error}",
                temporary.display()
            )
        })?;
        std::fs::rename(&temporary, path).map_err(|error| {
            format!(
                "failed to replace managed config {}: {error}",
                path.display()
            )
        })?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    write_result
}
