use super::{
    ASP_CODEX_PLUGIN_MARKETPLACE_NAME, ASP_CODEX_PLUGIN_NAME, CodexHookConfigInstallReceipt,
    asp_codex_plugin_version, codex_project_plugin_cache_path, global_codex_config_path,
    write_codex_config_atomically,
};
use std::{fs, path::Path};

pub(in crate::command) const ASP_CODEX_PLUGIN_MANIFEST_JSON: &str =
    include_str!("../../../../asp-codex-plugin/.codex-plugin/plugin.json");
pub(in crate::command) const ASP_CODEX_PLUGIN_HOOKS_JSON: &str =
    include_str!("../../../../asp-codex-plugin/hooks/hooks.json");
pub(in crate::command) const ASP_CODEX_PLUGIN_MARKETPLACE_JSON: &str =
    include_str!("../../../../.agents/plugins/marketplace.json");

pub(in crate::command) fn validate_codex_plugin_source_payload() -> Result<String, String> {
    let manifest: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MANIFEST_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin manifest JSON: {error}"))?;
    if manifest.get("hooks").and_then(serde_json::Value::as_str) != Some("./hooks/hooks.json") {
        return Err("ASP Codex plugin manifest must own ./hooks/hooks.json".to_string());
    }
    let marketplace: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_MARKETPLACE_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin marketplace JSON: {error}"))?;
    let marketplace_plugin = marketplace
        .get("plugins")
        .and_then(serde_json::Value::as_array)
        .and_then(|plugins| {
            plugins
                .iter()
                .find(|plugin| plugin["name"] == ASP_CODEX_PLUGIN_NAME)
        })
        .ok_or_else(|| "ASP Codex plugin marketplace entry is missing".to_string())?;
    if marketplace_plugin["source"]["source"].as_str() != Some("local")
        || marketplace_plugin["source"]["path"].as_str() != Some("./asp-codex-plugin")
    {
        return Err("ASP Codex plugin marketplace source must own ./asp-codex-plugin".to_string());
    }
    let hooks: serde_json::Value = serde_json::from_str(ASP_CODEX_PLUGIN_HOOKS_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin hooks JSON: {error}"))?;
    let events = hooks
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "ASP Codex plugin hooks JSON missing hooks event map".to_string())?;
    if events.len() != 8
        || events["PreToolUse"][0]["matcher"]
            .as_str()
            .map(str::trim)
            .filter(|matcher| !matcher.is_empty())
            .is_none()
    {
        return Err("ASP Codex plugin Hook payload is incomplete".to_string());
    }
    Ok(blake3::hash(
        format!(
            "{}\n{}\n{}",
            ASP_CODEX_PLUGIN_MANIFEST_JSON,
            ASP_CODEX_PLUGIN_HOOKS_JSON,
            ASP_CODEX_PLUGIN_MARKETPLACE_JSON
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string())
}

pub(in crate::command) fn remove_codex_managed_global_hook_config(
    config_path: &Path,
    project_config_path: &Path,
) -> Result<CodexHookConfigInstallReceipt, String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    agent_semantic_hook::validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    let cleaned = agent_semantic_hook::remove_codex_managed_hook_config(&existing);
    let cleaned =
        agent_semantic_hook::remove_codex_global_hook_trust_config(&cleaned, project_config_path);
    let cleaned = agent_semantic_hook::remove_codex_global_hook_trust_config(&cleaned, config_path);
    let cleaned = format!("{}\n", cleaned.trim_end());
    agent_semantic_hook::validate_codex_config_toml(&cleaned).map_err(|error| {
        format!("refusing to write invalid cleaned global Codex config TOML: {error}")
    })?;
    let digest = blake3::hash(cleaned.as_bytes()).to_hex().to_string();
    let changed = write_codex_config_atomically(config_path, cleaned.as_bytes())?;
    Ok(CodexHookConfigInstallReceipt { changed, digest })
}

pub(in crate::command) fn codex_plugin_hook_present(project_root: &Path) -> Result<bool, String> {
    let plugin_id = format!(
        "{}@{}",
        ASP_CODEX_PLUGIN_NAME, ASP_CODEX_PLUGIN_MARKETPLACE_NAME
    );
    let global_config_path = global_codex_config_path()?;
    let plugin_version = asp_codex_plugin_version()?;
    let global_enabled = agent_semantic_config::codex_config_plugin_enabled(
        &global_config_path,
        plugin_id.as_str().into(),
    )
    .map_err(|error| error.to_string())?;
    let global_payload = global_config_path
        .parent()
        .map(|codex_home| {
            codex_home
                .join("plugins")
                .join("cache")
                .join(ASP_CODEX_PLUGIN_MARKETPLACE_NAME)
                .join(ASP_CODEX_PLUGIN_NAME)
                .join(&plugin_version)
                .join("hooks")
                .join("hooks.json")
                .is_file()
        })
        .unwrap_or(false);

    let project_config_path = project_root.join(".codex").join("config.toml");
    let project_enabled = agent_semantic_config::codex_config_plugin_enabled(
        &project_config_path,
        plugin_id.as_str().into(),
    )
    .map_err(|error| error.to_string())?;
    let project_payload = codex_project_plugin_cache_path(project_root)?
        .join("hooks")
        .join("hooks.json")
        .is_file();

    Ok((global_enabled && global_payload) || (project_enabled && project_payload))
}
