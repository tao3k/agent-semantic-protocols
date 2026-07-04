//! Codex plugin installation path for `asp install plugin --codex`.

use super::hook_runtime_subagent::install_codex_resident_agents;
use agent_semantic_hook::{
    install_codex_user_project_trust, merge_codex_asp_explorer_role_config,
    validate_codex_config_toml,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ASP_CODEX_PLUGIN_NAME: &str = "asp-codex-plugin";
const ASP_CODEX_PLUGIN_MARKETPLACE_NAME: &str = "asp-project";
const ASP_CODEX_PLUGIN_MANIFEST_JSON: &str =
    include_str!("../../../../asp-codex-plugin/.codex-plugin/plugin.json");
const ASP_CODEX_PLUGIN_HOOKS_JSON: &str =
    include_str!("../../../../asp-codex-plugin/hooks/hooks.json");

#[derive(Clone, Copy)]
pub(super) enum CodexPluginScope {
    Project,
    Global,
}

impl CodexPluginScope {
    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

pub(super) fn codex_plugin_scope_arg(
    args: &[String],
    client: &str,
) -> Result<CodexPluginScope, String> {
    let global_plugin = args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--global" | "--global-plugin"));
    if global_plugin && client != "codex" {
        return Err("--global is only supported for Codex plugin installations".to_string());
    }
    Ok(if global_plugin {
        CodexPluginScope::Global
    } else {
        CodexPluginScope::Project
    })
}

pub(super) fn install_codex_plugin_hooks(
    project_root: &Path,
    scope: CodexPluginScope,
    subagent_model: &str,
) -> Result<(PathBuf, String), String> {
    let plugin_cache = ensure_codex_project_plugin_cache_static_files(project_root)?;
    let plugin_manifest = plugin_cache.join(".codex-plugin").join("plugin.json");
    let marketplace_name = ASP_CODEX_PLUGIN_MARKETPLACE_NAME;
    let project_config_path = install_codex_project_plugin_config(project_root)?;
    let trust_config_path = install_codex_user_project_trust(&project_config_path)?;
    let codex_agent_config_path = global_codex_config_path()?;
    let codex_agent_home = codex_agent_config_path
        .parent()
        .ok_or_else(|| "global Codex config path has no parent".to_string())?
        .to_path_buf();
    fs::create_dir_all(&codex_agent_home)
        .map_err(|error| format!("failed to create {}: {error}", codex_agent_home.display()))?;
    let subagent_path = install_codex_resident_agents(&codex_agent_home, subagent_model)?;
    let codex_home = match scope {
        CodexPluginScope::Project => Some(project_root.join(".codex")),
        CodexPluginScope::Global => None,
    };
    if let Some(codex_home) = codex_home.as_ref() {
        fs::create_dir_all(codex_home)
            .map_err(|error| format!("failed to create {}: {error}", codex_home.display()))?;
    }
    let plugin_id = format!("{ASP_CODEX_PLUGIN_NAME}@{marketplace_name}");
    let installed_path = match scope {
        CodexPluginScope::Project => {
            normalize_codex_project_marketplace_source(
                &project_config_path,
                marketplace_name,
                true,
            )?;
            ensure_codex_project_plugin_enabled(&project_config_path, &plugin_id)?;
            String::new()
        }
        CodexPluginScope::Global => {
            ensure_codex_plugin_marketplace_registered(
                project_root,
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
    };
    ensure_codex_asp_explorer_role_config(&codex_agent_config_path)?;
    let config_path = match scope {
        CodexPluginScope::Project => project_root.join(".codex").join("config.toml"),
        CodexPluginScope::Global => global_codex_config_path()?,
    };
    Ok((
        config_path,
        format!(
            " pluginScope={} pluginManifest={} pluginMarketplace={} projectConfig={} projectTrustConfig={} codexAgentConfig={} subagent={}{}",
            scope.label(),
            super::display_path(project_root, &plugin_manifest),
            marketplace_name,
            super::display_path(project_root, &project_config_path),
            super::display_path(project_root, &trust_config_path),
            super::display_path(project_root, &codex_agent_config_path),
            super::display_path(project_root, &subagent_path),
            installed_path,
        ),
    ))
}

fn write_codex_plugin_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, format!("{}\n", content.trim_end()).as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
}

pub(super) fn sync_codex_project_plugin_cache(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let cache_root = ensure_codex_project_plugin_cache_static_files(project_root)?;
    Ok(Some(cache_root))
}

fn ensure_codex_project_plugin_cache_static_files(project_root: &Path) -> Result<PathBuf, String> {
    let cache_root = codex_project_plugin_cache_path(project_root)?;
    if !cache_root.is_dir() {
        if cache_root.exists() {
            return Err(format!(
                "Codex plugin cache path {} exists but is not a directory",
                cache_root.display()
            ));
        }
        fs::create_dir_all(&cache_root)
            .map_err(|error| format!("failed to create {}: {error}", cache_root.display()))?;
    }
    write_codex_plugin_file(
        &cache_root.join(".codex-plugin").join("plugin.json"),
        ASP_CODEX_PLUGIN_MANIFEST_JSON,
    )?;
    write_codex_plugin_file(
        &cache_root.join("hooks").join("hooks.json"),
        ASP_CODEX_PLUGIN_HOOKS_JSON,
    )?;
    Ok(cache_root)
}

pub(super) fn codex_project_plugin_cache_skill_path(
    project_root: &Path,
) -> Result<PathBuf, String> {
    Ok(codex_project_plugin_cache_path(project_root)?.join(codex_plugin_skill_relative_path()))
}

pub(super) fn codex_project_plugin_hooks_present(project_root: &Path) -> bool {
    match codex_project_plugin_hooks_json_path(project_root) {
        Ok(path) => path.is_file(),
        Err(_) => false,
    }
}

pub(super) fn codex_project_plugin_hooks_json_path(project_root: &Path) -> Result<PathBuf, String> {
    Ok(codex_project_plugin_cache_path(project_root)?
        .join("hooks")
        .join("hooks.json"))
}

fn codex_project_plugin_cache_path(project_root: &Path) -> Result<PathBuf, String> {
    Ok(project_root.join(codex_project_plugin_cache_relative_path()?))
}

fn codex_project_plugin_cache_relative_path() -> Result<PathBuf, String> {
    Ok(Path::new(".codex")
        .join("plugins")
        .join("cache")
        .join(ASP_CODEX_PLUGIN_MARKETPLACE_NAME)
        .join(ASP_CODEX_PLUGIN_NAME)
        .join(asp_codex_plugin_version()?))
}

fn codex_plugin_skill_relative_path() -> PathBuf {
    Path::new("skills")
        .join("agent-semantic-protocols")
        .join("SKILL.org")
}

fn asp_codex_plugin_version() -> Result<String, String> {
    serde_json::from_str::<serde_json::Value>(ASP_CODEX_PLUGIN_MANIFEST_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin manifest JSON: {error}"))?
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| "ASP Codex plugin manifest JSON missing version".to_string())
}

fn install_codex_project_plugin_config(project_root: &Path) -> Result<PathBuf, String> {
    let codex_dir = project_root.join(".codex");
    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.is_file() {
        validate_codex_config_toml(&existing)
            .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    }
    let merged = normalize_codex_project_plugin_config(&existing);
    if merged != existing || !config_path.is_file() {
        validate_codex_config_toml(&merged).map_err(|error| {
            format!("refusing to write invalid Codex project plugin config TOML: {error}")
        })?;
        fs::write(&config_path, merged.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(config_path)
}

fn normalize_codex_project_plugin_config(content: &str) -> String {
    let content = content.trim();
    if content.is_empty() {
        String::new()
    } else {
        format!("{content}\n")
    }
}

fn ensure_codex_asp_explorer_role_config(config_path: &Path) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to update invalid Codex config TOML: {error}"))?;
    let merged = merge_codex_asp_explorer_role_config(&existing).map_err(|error| {
        format!("refusing to merge Codex ASP Explorer role registration: {error}")
    })?;
    if merged != existing {
        validate_codex_config_toml(&merged).map_err(|error| {
            format!("refusing to write invalid Codex ASP Explorer role config TOML: {error}")
        })?;
        fs::write(config_path, merged.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(())
}

fn ensure_codex_project_plugin_enabled(config_path: &Path, plugin_id: &str) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to update invalid Codex plugin config TOML: {error}"))?;
    let parsed = toml::from_str::<toml::Value>(&existing)
        .map_err(|error| format!("invalid Codex plugin config TOML: {error}"))?;
    let enabled = parsed
        .get("plugins")
        .and_then(toml::Value::as_table)
        .and_then(|plugins| plugins.get(plugin_id))
        .and_then(toml::Value::as_table)
        .and_then(|plugin| plugin.get("enabled"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false);
    if enabled {
        return Ok(());
    }
    let content = remove_codex_project_plugin_section(&existing, plugin_id);
    let plugin_section = format!("[plugins.{}]\nenabled = true", toml_basic_string(plugin_id));
    let content = content.trim_end();
    let merged = if content.is_empty() {
        format!("{plugin_section}\n")
    } else {
        format!("{content}\n\n{plugin_section}\n")
    };
    validate_codex_config_toml(&merged)
        .map_err(|error| format!("refusing to write invalid Codex plugin config TOML: {error}"))?;
    fs::write(config_path, merged.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    Ok(())
}

fn remove_codex_project_plugin_section(existing: &str, plugin_id: &str) -> String {
    let section_plain = format!("[plugins.{plugin_id}]");
    let section_quoted = format!("[plugins.{}]", toml_basic_string(plugin_id));
    remove_toml_sections(existing, &[section_plain.as_str(), section_quoted.as_str()])
}

fn remove_toml_sections(existing: &str, sections: &[&str]) -> String {
    let mut lines = Vec::new();
    let mut skipping = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = sections.contains(&trimmed);
            if skipping {
                continue;
            }
        }
        if !skipping {
            lines.push(line.to_string());
        }
    }
    format!("{}\n", lines.join("\n").trim_end())
}

fn normalize_codex_project_marketplace_source(
    config_path: &Path,
    marketplace_name: &str,
    create_if_missing: bool,
) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to normalize invalid Codex config TOML: {error}"))?;
    let section_plain = format!("[marketplaces.{marketplace_name}]");
    let section_quoted = format!("[marketplaces.{}]", toml_basic_string(marketplace_name));
    let mut lines = Vec::new();
    let mut in_section = false;
    let mut saw_section = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if trimmed == section_plain || trimmed == section_quoted {
                saw_section = true;
                in_section = true;
                lines.push(section_plain.clone());
                lines.push("source_type = \"local\"".to_string());
                lines.push("source = \".\"".to_string());
                continue;
            }
            in_section = false;
        }
        if in_section {
            continue;
        }
        lines.push(line.to_string());
    }
    if !saw_section && !create_if_missing {
        return Ok(());
    }
    if !saw_section {
        if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push(section_plain);
        lines.push("source_type = \"local\"".to_string());
        lines.push("source = \".\"".to_string());
    }
    let normalized = format!("{}\n", lines.join("\n").trim_end());
    if normalized != existing {
        validate_codex_config_toml(&normalized).map_err(|error| {
            format!("refusing to write invalid normalized Codex config TOML: {error}")
        })?;
        fs::write(config_path, normalized.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(())
}

fn ensure_codex_plugin_marketplace_registered(
    project_root: &Path,
    codex_home: Option<&Path>,
    marketplace_name: &str,
) -> Result<(), String> {
    let source = project_root.to_str().unwrap_or(".").to_string();
    let add_args = [
        "plugin".to_string(),
        "marketplace".to_string(),
        "add".to_string(),
        source,
        "--json".to_string(),
    ];
    match run_codex_plugin_command(&add_args, project_root, codex_home) {
        Ok(_) => Ok(()),
        Err(add_error) if add_error.contains("already added from a different source") => {
            if codex_marketplace_points_to_project_root(project_root, codex_home, marketplace_name)
                .map_err(|list_error| {
                    format!(
                        "{add_error}; additionally failed to inspect existing marketplace root: {list_error}"
                    )
                })?
            {
                Ok(())
            } else {
                Err(add_error)
            }
        }
        Err(error) => Err(error),
    }
}

fn codex_marketplace_points_to_project_root(
    project_root: &Path,
    codex_home: Option<&Path>,
    marketplace_name: &str,
) -> Result<bool, String> {
    let stdout = run_codex_plugin_command(
        &[
            "plugin".to_string(),
            "marketplace".to_string(),
            "list".to_string(),
            "--json".to_string(),
        ],
        project_root,
        codex_home,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("invalid codex plugin marketplace list JSON: {error}"))?;
    let project_root = fs::canonicalize(project_root)
        .map_err(|error| format!("failed to resolve {}: {error}", project_root.display()))?;
    let marketplaces = value
        .get("marketplaces")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "codex plugin marketplace list JSON missing marketplaces".to_string())?;
    for marketplace in marketplaces {
        let Some(name) = marketplace.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if name != marketplace_name {
            continue;
        }
        let Some(root) = marketplace.get("root").and_then(serde_json::Value::as_str) else {
            return Ok(false);
        };
        let root = fs::canonicalize(root)
            .map_err(|error| format!("failed to resolve marketplace root {root}: {error}"))?;
        return Ok(root == project_root);
    }
    Ok(false)
}

fn run_codex_plugin_command(
    args: &[String],
    cwd: &Path,
    codex_home: Option<&Path>,
) -> Result<String, String> {
    let mut command = Command::new("codex");
    command.args(args).current_dir(cwd);
    if let Some(codex_home) = codex_home {
        command.env("CODEX_HOME", codex_home);
    }
    let output = command
        .output()
        .map_err(|error| format!("failed to run codex {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "codex {} failed: stdout={} stderr={}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn codex_plugin_installed_path(add_stdout: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(add_stdout)
        .ok()
        .and_then(|value| {
            value
                .get("installedPath")
                .and_then(serde_json::Value::as_str)
                .map(ToString::to_string)
        })
}

fn toml_basic_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn global_codex_config_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join("config.toml"));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex").join("config.toml"))
        .ok_or_else(|| "missing CODEX_HOME and HOME; cannot locate Codex config".to_string())
}
