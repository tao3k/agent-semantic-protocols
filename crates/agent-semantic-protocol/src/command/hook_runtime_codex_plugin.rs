//! Codex plugin install path for `asp hook install`.

use super::hook_runtime_subagent::install_codex_asp_explorer_agent;
use agent_semantic_hook::{
    merge_codex_asp_explorer_role_config, remove_codex_managed_hook_blocks,
    validate_codex_config_toml,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ASP_CODEX_PLUGIN_NAME: &str = "asp-codex-plugin";
const ASP_CODEX_PLUGIN_MARKETPLACE_NAME: &str = "asp-project";

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
    let global_plugin = args.iter().any(|arg| arg == "--global-plugin");
    if global_plugin && client != "codex" {
        return Err("--global-plugin is only supported with --client codex".to_string());
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
    let plugin_manifest = project_root
        .join(ASP_CODEX_PLUGIN_NAME)
        .join(".codex-plugin")
        .join("plugin.json");
    if !plugin_manifest.is_file() {
        return Err(format!(
            "Codex plugin install requires {}",
            super::display_path(project_root, &plugin_manifest)
        ));
    }
    let (marketplace_path, marketplace_name) =
        ensure_codex_project_plugin_marketplace(project_root)?;
    let cleaned_config_path = cleanup_legacy_codex_project_hook_config(project_root)?;
    let subagent_path = install_codex_asp_explorer_agent(project_root, subagent_model)?;
    let codex_home = match scope {
        CodexPluginScope::Project => Some(project_root.join(".codex")),
        CodexPluginScope::Global => None,
    };
    if let Some(codex_home) = codex_home.as_ref() {
        fs::create_dir_all(codex_home)
            .map_err(|error| format!("failed to create {}: {error}", codex_home.display()))?;
    }
    ensure_codex_plugin_marketplace_registered(
        project_root,
        codex_home.as_deref(),
        &marketplace_name,
    )?;
    let add_stdout = run_codex_plugin_command(
        &[
            "plugin".to_string(),
            "add".to_string(),
            format!("{ASP_CODEX_PLUGIN_NAME}@{marketplace_name}"),
            "--json".to_string(),
        ],
        project_root,
        codex_home.as_deref(),
    )?;
    ensure_codex_asp_explorer_role_config(&cleaned_config_path)?;
    let installed_path = codex_plugin_installed_path(&add_stdout)
        .map(|path| format!(" pluginInstalledPath={path}"))
        .unwrap_or_default();
    let config_path = match scope {
        CodexPluginScope::Project => project_root.join(".codex").join("config.toml"),
        CodexPluginScope::Global => global_codex_config_path()?,
    };
    Ok((
        config_path,
        format!(
            " pluginScope={} pluginMarketplace={} pluginMarketplaceConfig={} legacyHookConfigCleaned={} subagent={}{}",
            scope.label(),
            marketplace_name,
            super::display_path(project_root, &marketplace_path),
            super::display_path(project_root, &cleaned_config_path),
            super::display_path(project_root, &subagent_path),
            installed_path,
        ),
    ))
}

fn cleanup_legacy_codex_project_hook_config(project_root: &Path) -> Result<PathBuf, String> {
    let codex_dir = project_root.join(".codex");
    fs::create_dir_all(&codex_dir)
        .map_err(|error| format!("failed to create {}: {error}", codex_dir.display()))?;
    let config_path = codex_dir.join("config.toml");
    let existing = fs::read_to_string(&config_path).unwrap_or_default();
    if config_path.is_file() {
        validate_codex_config_toml(&existing)
            .map_err(|error| format!("refusing to clean invalid Codex config TOML: {error}"))?;
    }
    let cleaned = remove_codex_managed_hook_blocks(&existing);
    if cleaned != existing.trim() {
        let contents = if cleaned.trim().is_empty() {
            String::new()
        } else {
            format!("{}\n", cleaned.trim_end())
        };
        validate_codex_config_toml(&contents).map_err(|error| {
            format!("refusing to write invalid cleaned Codex config TOML: {error}")
        })?;
        fs::write(&config_path, contents.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", config_path.display()))?;
    }
    Ok(config_path)
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

fn ensure_codex_project_plugin_marketplace(
    project_root: &Path,
) -> Result<(PathBuf, String), String> {
    let marketplace_path = project_root
        .join(".agents")
        .join("plugins")
        .join("marketplace.json");
    if let Some(parent) = marketplace_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let mut root = if marketplace_path.is_file() {
        let content = fs::read_to_string(&marketplace_path)
            .map_err(|error| format!("failed to read {}: {error}", marketplace_path.display()))?;
        serde_json::from_str::<serde_json::Value>(&content)
            .map_err(|error| format!("invalid {}: {error}", marketplace_path.display()))?
    } else {
        serde_json::json!({
            "name": ASP_CODEX_PLUGIN_MARKETPLACE_NAME,
            "interface": {
                "displayName": "ASP Project"
            },
            "plugins": []
        })
    };
    let root_object = root
        .as_object_mut()
        .ok_or_else(|| format!("{} must contain a JSON object", marketplace_path.display()))?;
    let marketplace_name = match root_object.get("name").and_then(serde_json::Value::as_str) {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        Some(_) => return Err(format!("{} has an empty name", marketplace_path.display())),
        None => {
            root_object.insert(
                "name".to_string(),
                serde_json::Value::String(ASP_CODEX_PLUGIN_MARKETPLACE_NAME.to_string()),
            );
            ASP_CODEX_PLUGIN_MARKETPLACE_NAME.to_string()
        }
    };
    root_object
        .entry("interface")
        .or_insert_with(|| serde_json::json!({"displayName": "ASP Project"}));
    let plugins = root_object
        .entry("plugins")
        .or_insert_with(|| serde_json::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| format!("{} plugins must be an array", marketplace_path.display()))?;
    plugins.retain(|plugin| {
        plugin.get("name").and_then(serde_json::Value::as_str) != Some(ASP_CODEX_PLUGIN_NAME)
    });
    plugins.push(serde_json::json!({
        "name": ASP_CODEX_PLUGIN_NAME,
        "source": {
            "source": "local",
            "path": format!("./{ASP_CODEX_PLUGIN_NAME}")
        },
        "policy": {
            "installation": "AVAILABLE",
            "authentication": "ON_INSTALL"
        },
        "category": "Productivity"
    }));
    let rendered = serde_json::to_string_pretty(&root)
        .map_err(|error| format!("failed to render {}: {error}", marketplace_path.display()))?;
    fs::write(&marketplace_path, format!("{rendered}\n").as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", marketplace_path.display()))?;
    Ok((marketplace_path, marketplace_name))
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

fn global_codex_config_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join("config.toml"));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex").join("config.toml"))
        .ok_or_else(|| "missing CODEX_HOME and HOME; cannot locate Codex config".to_string())
}
