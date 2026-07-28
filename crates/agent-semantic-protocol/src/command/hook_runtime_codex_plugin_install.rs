use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    ASP_CODEX_PLUGIN_MARKETPLACE_NAME, ASP_CODEX_PLUGIN_NAME, CodexPluginScope,
    codex_plugin_hook_key_source, codex_plugin_installed_path, codex_plugin_source_root,
    display_codex_plugin_source_root, ensure_codex_global_plugin_cache_static_files,
    ensure_codex_plugin_cache_static_files, ensure_codex_plugin_marketplace_registered,
    ensure_codex_project_plugin_cache_static_files, ensure_codex_project_plugin_enabled,
    global_codex_config_path, install_codex_project_plugin_config, install_codex_resident_agents,
    plugin_path, remove_codex_project_marketplace_source, remove_codex_project_plugin_config,
    remove_codex_user_managed_hook_config, run_codex_plugin_command,
};
use crate::command::hook_runtime::display_path;

pub(in crate::command) fn install_codex_plugin_hooks(
    project_root: &Path,
    scope: CodexPluginScope,
    subagent_model: &str,
    asp_binary_path: &Path,
) -> Result<(PathBuf, String), String> {
    plugin_path::require_current(asp_binary_path)?;
    let marketplace_name = ASP_CODEX_PLUGIN_MARKETPLACE_NAME;
    let plugin_source_root = codex_plugin_source_root(project_root)?;
    let project_config_path = install_codex_project_plugin_config(project_root)?;
    let codex_agent_config_path = global_codex_config_path()?;
    remove_codex_user_managed_hook_config(&codex_agent_config_path, &project_config_path)?;
    let codex_agent_home = codex_agent_config_path
        .parent()
        .ok_or_else(|| "global Codex config path has no parent".to_string())?
        .to_path_buf();
    fs::create_dir_all(&codex_agent_home)
        .map_err(|error| format!("failed to create {}: {error}", codex_agent_home.display()))?;
    let plugin_id = format!("{ASP_CODEX_PLUGIN_NAME}@{marketplace_name}");
    let plugin_hook_key_source = codex_plugin_hook_key_source();
    let plugin_source_trust_config = agent_semantic_hook::install_codex_user_plugin_trust_state(
        &plugin_source_root
            .join(ASP_CODEX_PLUGIN_NAME)
            .join("hooks")
            .join("hooks.json"),
        &plugin_hook_key_source,
    )
    .map(|path| display_path(project_root, &path))
    .unwrap_or_else(|error| format!("skipped:{error}"));
    let global_plugin_cache = ensure_codex_global_plugin_cache_static_files(&codex_agent_home)?;
    let plugin_cache_trust_config = agent_semantic_hook::install_codex_user_plugin_trust_state(
        &global_plugin_cache.join("hooks").join("hooks.json"),
        &plugin_hook_key_source,
    )?;
    let subagent_path = install_codex_resident_agents(&codex_agent_home, subagent_model)?;
    let project_plugin_cache = match scope {
        CodexPluginScope::Project => Some(ensure_codex_project_plugin_cache_static_files(
            project_root,
        )?),
        CodexPluginScope::Global => None,
    };
    let codex_home = match scope {
        CodexPluginScope::Project => Some(project_root.join(".codex")),
        CodexPluginScope::Global => None,
    };
    if let Some(codex_home) = codex_home.as_ref() {
        fs::create_dir_all(codex_home)
            .map_err(|error| format!("failed to create {}: {error}", codex_home.display()))?;
    }
    let installed_path = match scope {
        CodexPluginScope::Project => {
            ensure_codex_plugin_marketplace_registered(
                project_root,
                &plugin_source_root,
                None,
                marketplace_name,
            )?;
            remove_codex_project_marketplace_source(&project_config_path, marketplace_name)?;
            ensure_codex_project_plugin_enabled(&project_config_path, &plugin_id)?;
            String::new()
        }
        CodexPluginScope::Global => {
            remove_codex_project_marketplace_source(&project_config_path, marketplace_name)?;
            remove_codex_project_plugin_config(&project_config_path, &plugin_id)?;
            if agent_semantic_config::codex_config_plugin_enabled(
                &codex_agent_config_path,
                plugin_id.as_str().into(),
            )
            .map_err(|error| error.to_string())?
            {
                format!(
                    " pluginInstalledPath={}",
                    display_path(project_root, &global_plugin_cache)
                )
            } else {
                ensure_codex_plugin_marketplace_registered(
                    project_root,
                    &plugin_source_root,
                    codex_home.as_deref(),
                    marketplace_name,
                )?;
                let add_stdout = run_codex_plugin_command(
                    &[
                        "plugin".to_string(),
                        "add".to_string(),
                        plugin_id,
                        "--json".to_string(),
                    ],
                    project_root,
                    codex_home.as_deref(),
                )?;
                codex_plugin_installed_path(&add_stdout)
                    .map(|path| format!(" pluginInstalledPath={path}"))
                    .unwrap_or_default()
            }
        }
    };
    ensure_codex_plugin_cache_static_files(&global_plugin_cache)?;
    let config_path = match scope {
        CodexPluginScope::Project => project_root.join(".codex").join("config.toml"),
        CodexPluginScope::Global => global_codex_config_path()?,
    };
    let project_plugin_cache_fields = project_plugin_cache
        .as_ref()
        .map(|plugin_cache| {
            format!(
                " pluginManifest={} pluginCache={}",
                display_path(
                    project_root,
                    &plugin_cache.join(".codex-plugin").join("plugin.json")
                ),
                display_path(project_root, plugin_cache),
            )
        })
        .unwrap_or_default();
    Ok((
        config_path,
        format!(
            " pluginScope={}{} pluginMarketplace={} pluginMarketplaceSource={} projectConfig={} codexAgentConfig={} pluginSourceTrustConfig={} pluginCacheTrustConfig={} subagent={} globalPluginCache={}{}",
            scope.label(),
            project_plugin_cache_fields,
            marketplace_name,
            display_codex_plugin_source_root(project_root, &plugin_source_root),
            display_path(project_root, &project_config_path),
            display_path(project_root, &codex_agent_config_path),
            plugin_source_trust_config,
            display_path(project_root, &plugin_cache_trust_config),
            display_path(project_root, &subagent_path),
            display_path(project_root, &global_plugin_cache),
            installed_path,
        ),
    ))
}
