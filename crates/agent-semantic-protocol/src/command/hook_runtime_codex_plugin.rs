//! Codex plugin installation path for `asp install plugin --codex`.

use super::hook_runtime_subagent::install_codex_resident_agents;
use agent_semantic_hook::validate_codex_config_toml;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[path = "hook_runtime_codex_plugin_path.rs"]
mod plugin_path;

#[path = "hook_runtime_codex_plugin_install.rs"]
mod install;
pub(super) use install::install_codex_plugin_hooks;

fn remove_codex_user_managed_hook_config(
    config_path: &Path,
    project_config_path: &Path,
) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    let cleaned = agent_semantic_hook::remove_codex_managed_hook_config(&existing);
    let cleaned =
        agent_semantic_hook::remove_codex_global_hook_trust_config(&cleaned, project_config_path);
    let cleaned = agent_semantic_hook::remove_codex_global_hook_trust_config(&cleaned, config_path);
    if cleaned != existing {
        validate_codex_config_toml(&cleaned).map_err(|error| {
            format!("refusing to write invalid cleaned Codex config TOML: {error}")
        })?;
        fs::write(config_path, cleaned.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(())
}

fn write_codex_plugin_file(path: &Path, content: &str) -> Result<(), String> {
    let desired = format!("{}\n", content.trim_end());
    if path.is_file() {
        let existing = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if existing == desired.as_bytes() {
            return Ok(());
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, desired.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(())
}

const ASP_CODEX_PLUGIN_HOOKS_SCHEMA_JSON: &str =
    include_str!("../../../../schemas/codex-plugin-hooks.v1.schema.json");

fn validate_codex_plugin_hooks_manifest(hooks: &serde_json::Value) -> Result<(), String> {
    let schema = serde_json::from_str::<serde_json::Value>(ASP_CODEX_PLUGIN_HOOKS_SCHEMA_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin hooks v1 schema: {error}"))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|error| format!("invalid ASP Codex plugin hooks v1 schema: {error}"))?;
    let errors = validator
        .iter_errors(hooks)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn render_codex_plugin_hooks_json() -> Result<String, String> {
    let hooks = serde_json::from_str::<serde_json::Value>(ASP_CODEX_PLUGIN_HOOKS_JSON)
        .map_err(|error| format!("invalid ASP Codex plugin hooks JSON: {error}"))?;
    validate_codex_plugin_hooks_manifest(&hooks)
        .map_err(|error| format!("invalid ASP Codex plugin hooks manifest: {error}"))?;
    serde_json::to_string_pretty(&hooks)
        .map_err(|error| format!("failed to render ASP Codex plugin hooks JSON: {error}"))
}

#[cfg(test)]
#[path = "../../tests/unit/canonical_codex_plugin_hook_binary.rs"]
mod canonical_codex_plugin_hook_binary_tests;

fn validate_codex_plugin_manifest_hooks_path(plugin_root: &Path) -> Result<(), String> {
    let manifest_path = plugin_root.join(".codex-plugin").join("plugin.json");
    let manifest = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("failed to read {}: {error}", manifest_path.display()))?;
    let manifest = serde_json::from_str::<serde_json::Value>(&manifest)
        .map_err(|error| format!("invalid ASP Codex plugin manifest JSON: {error}"))?;
    let hooks_path = manifest
        .get("hooks")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "ASP Codex plugin manifest `hooks` must be a string path".to_string())?;
    if hooks_path.trim().is_empty() {
        return Err("ASP Codex plugin manifest `hooks` path must not be empty".to_string());
    }
    if !hooks_path.starts_with("./") {
        return Err(
            "ASP Codex plugin manifest `hooks` path must start with `./` relative to plugin root"
                .to_string(),
        );
    }
    let resolved_hooks_path = plugin_root.join(hooks_path);
    if !resolved_hooks_path.is_file() {
        return Err(format!(
            "ASP Codex plugin manifest `hooks` path does not resolve to a file: {}",
            resolved_hooks_path.display()
        ));
    }
    Ok(())
}

pub(super) fn sync_codex_project_plugin_cache(
    project_root: &Path,
) -> Result<Option<PathBuf>, String> {
    let cache_root = ensure_codex_project_plugin_cache_static_files(project_root)?;
    Ok(Some(cache_root))
}

fn ensure_codex_project_plugin_cache_static_files(project_root: &Path) -> Result<PathBuf, String> {
    ensure_codex_plugin_cache_static_files(&codex_project_plugin_cache_path(project_root)?)
}

fn ensure_codex_global_plugin_cache_static_files(codex_home: &Path) -> Result<PathBuf, String> {
    let cache_root = codex_home
        .join("plugins")
        .join("cache")
        .join(ASP_CODEX_PLUGIN_MARKETPLACE_NAME)
        .join(ASP_CODEX_PLUGIN_NAME)
        .join(asp_codex_plugin_version()?);
    ensure_codex_plugin_cache_static_files(&cache_root)
}

fn ensure_codex_plugin_cache_static_files(cache_root: &Path) -> Result<PathBuf, String> {
    if !cache_root.is_dir() {
        if cache_root.exists() {
            return Err(format!(
                "Codex plugin cache path {} exists but is not a directory",
                cache_root.display()
            ));
        }
        fs::create_dir_all(cache_root)
            .map_err(|error| format!("failed to create {}: {error}", cache_root.display()))?;
    }
    write_codex_plugin_file(
        &cache_root.join(".codex-plugin").join("plugin.json"),
        ASP_CODEX_PLUGIN_MANIFEST_JSON,
    )?;
    write_codex_plugin_file(
        &cache_root.join("hooks").join("hooks.json"),
        &render_codex_plugin_hooks_json()?,
    )?;
    validate_codex_plugin_manifest_hooks_path(cache_root)?;
    Ok(cache_root.to_path_buf())
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

pub(super) fn codex_global_plugin_hooks_present() -> bool {
    match codex_global_plugin_hooks_json_path() {
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

pub(crate) fn codex_plugin_source_root(project_root: &Path) -> Result<PathBuf, String> {
    if is_codex_plugin_source_root(project_root) {
        return fs::canonicalize(project_root)
            .map_err(|error| format!("failed to resolve {}: {error}", project_root.display()));
    }
    Err(format!(
        "PROJECT_ROOT {} is not an ASP Codex plugin marketplace source root: missing {}/.codex-plugin/plugin.json",
        project_root.display(),
        ASP_CODEX_PLUGIN_NAME,
    ))
}

fn is_codex_plugin_source_root(root: &Path) -> bool {
    root.join(ASP_CODEX_PLUGIN_NAME)
        .join(".codex-plugin")
        .join("plugin.json")
        .is_file()
}

fn display_codex_plugin_source_root(project_root: &Path, plugin_source_root: &Path) -> String {
    let display = super::display_path(project_root, plugin_source_root);
    if display.is_empty() {
        ".".to_string()
    } else {
        display
    }
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
    let merged = agent_semantic_hook::remove_codex_managed_hook_config(&merged);
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
    let mut lines = content
        .trim()
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();
    ensure_codex_project_feature_flags(&mut lines, &["hooks", "plugins", "unified_exec"]);
    let normalized = lines.join("\n");
    if normalized.trim().is_empty() {
        String::new()
    } else {
        format!("{}\n", normalized.trim_end())
    }
}

fn ensure_codex_project_feature_flags(lines: &mut Vec<String>, required_features: &[&str]) {
    let Some((features_start, features_end)) = codex_features_section_bounds(lines) else {
        if !lines.is_empty() && lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.push("[features]".to_string());
        lines.extend(
            required_features
                .iter()
                .map(|feature| format!("{feature} = true")),
        );
        return;
    };

    let required = required_features
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut present = std::collections::HashSet::new();
    lines[features_start + 1..features_end]
        .iter_mut()
        .filter_map(|line| {
            let key = line.trim_start().split_once('=')?.0.trim();
            required.get(key).copied().map(|feature| (line, feature))
        })
        .for_each(|(line, feature)| {
            let indent = line
                .chars()
                .take_while(|character| character.is_whitespace())
                .collect::<String>();
            *line = format!("{indent}{feature} = true");
            present.insert(feature);
        });
    let missing = required_features
        .iter()
        .copied()
        .filter(|feature| !present.contains(feature))
        .map(|feature| format!("{feature} = true"))
        .collect::<Vec<_>>();
    lines.splice(features_end..features_end, missing);
}

fn codex_features_section_bounds(lines: &[String]) -> Option<(usize, usize)> {
    let mut features_start = None;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !toml_table_header(trimmed) {
            continue;
        }
        if trimmed == "[features]" {
            features_start = Some(index);
            continue;
        }
        if let Some(start) = features_start {
            return Some((start, index));
        }
    }
    features_start.map(|start| (start, lines.len()))
}

fn toml_table_header(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.starts_with("[[")
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

fn remove_codex_project_plugin_config(config_path: &Path, plugin_id: &str) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    let cleaned = remove_codex_project_plugin_section(&existing, plugin_id);
    if cleaned != existing {
        validate_codex_config_toml(&cleaned).map_err(|error| {
            format!("refusing to write invalid cleaned Codex config TOML: {error}")
        })?;
        fs::write(config_path, cleaned.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(())
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

fn remove_codex_project_marketplace_source(
    config_path: &Path,
    marketplace_name: &str,
) -> Result<(), String> {
    let existing = fs::read_to_string(config_path).unwrap_or_default();
    validate_codex_config_toml(&existing)
        .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    let section_plain = format!("[marketplaces.{marketplace_name}]");
    let section_quoted = format!("[marketplaces.{}]", toml_basic_string(marketplace_name));
    let cleaned = remove_toml_sections(
        &existing,
        &[section_plain.as_str(), section_quoted.as_str()],
    );
    if cleaned != existing {
        validate_codex_config_toml(&cleaned).map_err(|error| {
            format!("refusing to write invalid cleaned Codex config TOML: {error}")
        })?;
        fs::write(config_path, cleaned.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(())
}

fn ensure_codex_plugin_marketplace_registered(
    command_cwd: &Path,
    plugin_source_root: &Path,
    codex_home: Option<&Path>,
    marketplace_name: &str,
) -> Result<(), String> {
    let source = plugin_source_root.to_str().unwrap_or(".").to_string();
    let add_args = [
        "plugin".to_string(),
        "marketplace".to_string(),
        "add".to_string(),
        source,
        "--json".to_string(),
    ];
    match run_codex_plugin_command(&add_args, command_cwd, codex_home) {
        Ok(_) => Ok(()),
        Err(add_error) if add_error.contains("already added from a different source") => {
            if codex_marketplace_points_to_source_root(
                command_cwd,
                plugin_source_root,
                codex_home,
                marketplace_name,
            )
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

fn codex_marketplace_points_to_source_root(
    command_cwd: &Path,
    plugin_source_root: &Path,
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
        command_cwd,
        codex_home,
    )?;
    let value = serde_json::from_str::<serde_json::Value>(&stdout)
        .map_err(|error| format!("invalid codex plugin marketplace list JSON: {error}"))?;
    let plugin_source_root = fs::canonicalize(plugin_source_root).map_err(|error| {
        format!(
            "failed to resolve {}: {error}",
            plugin_source_root.display()
        )
    })?;
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
        return Ok(root == plugin_source_root);
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
use super::hook_runtime_codex_plugin_identity::{
    codex_global_plugin_hooks_json_path, codex_plugin_hook_key_source,
};
