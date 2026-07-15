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
    if client_config_path.is_file() {
        load_client_config(&client_config_path)
            .map_err(|error| format!("invalid user hook config: {error}"))?;
    }
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
    let user_config_receipt = if client_config_path.is_file() {
        format!(
            " userConfig={} userConfigStatus=present",
            display_path(&project_root, &client_config_path)
        )
    } else {
        " userConfigStatus=missing".to_string()
    };
    let binary_paths = binary_install
        .paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "[{receipt_label}] client={client} activation={} activationRuntime=derived activationSync={}{} agentConfig={} orgState={} orgStateSync={} config={}{}{}{}{} binary=asp binaryPath={} binaryPaths={} binaryInstall={} binaryArtifactDigest={} binarySwitch=atomic mode=updated",
        display_path(&project_root, &activation_path),
        activation_status,
        user_config_receipt,
        display_path(&project_root, &agent_config_path),
        display_path(&project_root, &runtime_state.protocol_home.join("org")),
        org_state_sync.status,
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
