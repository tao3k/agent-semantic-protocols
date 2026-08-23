use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    ASP_CODEX_PLUGIN_MARKETPLACE_NAME, ASP_CODEX_PLUGIN_NAME, codex_plugin_installed_path,
    codex_plugin_source_root, display_codex_plugin_source_root,
    ensure_codex_plugin_marketplace_registered, global_codex_config_path, plugin_path,
    run_codex_plugin_command,
};
use crate::command::hook_runtime::display_path;

pub(in crate::command) fn install_codex_plugin_hooks(
    project_root: &Path,
    asp_binary_path: &Path,
) -> Result<(PathBuf, String), String> {
    plugin_path::require_current(asp_binary_path)?;
    let marketplace_name = ASP_CODEX_PLUGIN_MARKETPLACE_NAME;
    let plugin_source_root = codex_plugin_source_root(project_root)?;
    let codex_agent_config_path = global_codex_config_path()?;
    let existing_codex_agent_config =
        fs::read_to_string(&codex_agent_config_path).unwrap_or_default();
    if codex_agent_config_path.is_file() {
        agent_semantic_hook::validate_codex_config_toml(&existing_codex_agent_config).map_err(
            |error| {
                format!(
                    "refusing to install against invalid global Codex config {}: {error}",
                    codex_agent_config_path.display()
                )
            },
        )?;
    }
    let codex_agent_home = codex_agent_config_path
        .parent()
        .ok_or_else(|| "global Codex config path has no parent".to_string())?
        .to_path_buf();
    fs::create_dir_all(&codex_agent_home)
        .map_err(|error| format!("failed to create {}: {error}", codex_agent_home.display()))?;
    let plugin_id = format!("{ASP_CODEX_PLUGIN_NAME}@{marketplace_name}");
    let plugin_payload_digest = crate::command::hook_runtime::hook_runtime_codex_plugin::
        validate_codex_plugin_source_payload()?;
    let (installed_path, plugin_install_status) = {
        ensure_codex_plugin_marketplace_registered(
            project_root,
            &plugin_source_root,
            None,
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
            None,
        )?;
        let installed_path = codex_plugin_installed_path(&add_stdout)
            .map(|path| format!(" pluginInstalledPath={path}"))
            .unwrap_or_default();
        (installed_path, "updated")
    };
    let hook_config = crate::command::hook_runtime::hook_runtime_codex_plugin::
        remove_codex_managed_global_hook_config(
            &codex_agent_config_path,
        )?;
    let config_path = global_codex_config_path()?;
    Ok((
        config_path,
        format!(
            " pluginScope=global pluginMarketplace={} pluginMarketplaceSource={} pluginInstallStatus={} pluginPayloadStatus=validated pluginPayloadDigest={} codexAgentConfig={} hookAuthority=codex-plugin nativeInlineHookRemoved={} nativeInlineHookCleanupStatus={} configDigest={} agentConfigSync=not-on-plugin-install{}",
            marketplace_name,
            display_codex_plugin_source_root(project_root, &plugin_source_root),
            plugin_install_status,
            plugin_payload_digest,
            display_path(project_root, &codex_agent_config_path),
            display_path(project_root, &codex_agent_config_path),
            if hook_config.changed {
                "updated"
            } else {
                "reused"
            },
            hook_config.digest,
            installed_path,
        ),
    ))
}
