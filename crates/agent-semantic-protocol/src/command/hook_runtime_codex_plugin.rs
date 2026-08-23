//! Codex plugin installation path for `asp install plugin --codex`.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const ASP_CODEX_PLUGIN_NAME: &str = "asp-codex-plugin";
const ASP_CODEX_PLUGIN_MARKETPLACE_NAME: &str = "asp-project";
#[path = "hook_runtime_codex_plugin_authority.rs"]
mod authority;
#[cfg(test)]
pub(in crate::command) use authority::{
    ASP_CODEX_PLUGIN_HOOK_LAUNCHER, ASP_CODEX_PLUGIN_HOOKS_JSON, ASP_CODEX_PLUGIN_MANIFEST_JSON,
    ASP_CODEX_PLUGIN_MARKETPLACE_JSON,
};
pub(in crate::command) use authority::{
    remove_codex_managed_global_hook_config, validate_codex_plugin_source_payload,
};

#[cfg(test)]
#[path = "../../tests/unit/plugin_hook_authority.rs"]
mod plugin_hook_authority_tests;
static CODEX_CONFIG_PUBLISH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) struct CodexHookConfigInstallReceipt {
    pub(super) changed: bool,
    pub(super) digest: String,
}

#[path = "hook_runtime_codex_plugin_path.rs"]
mod plugin_path;

#[path = "hook_runtime_codex_plugin_install.rs"]
mod install;
pub(super) use install::install_codex_plugin_hooks;

fn write_codex_config_atomically(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    if path.is_file() {
        let existing = fs::read(path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if blake3::hash(&existing) == blake3::hash(bytes) {
            return Ok(false);
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("Codex config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let sequence = CODEX_CONFIG_PUBLISH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(
        ".config.toml.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let publish = (|| -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
        file.write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("failed to sync {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path).map_err(|error| {
            format!(
                "failed to publish {} to {}: {error}",
                temporary.display(),
                path.display()
            )
        })?;
        Ok(())
    })();
    if publish.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    publish.map(|()| true)
}

#[cfg(test)]
#[path = "../../tests/unit/canonical_codex_plugin_hook_binary.rs"]
mod canonical_codex_plugin_hook_binary_tests;

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

fn ensure_codex_plugin_marketplace_registered(
    command_cwd: &Path,
    plugin_source_root: &Path,
    codex_home: Option<&Path>,
    marketplace_name: &str,
) -> Result<(), String> {
    if codex_marketplace_points_to_source_root(
        command_cwd,
        plugin_source_root,
        codex_home,
        marketplace_name,
    )? {
        return Ok(());
    }
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

fn global_codex_config_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path).join("config.toml"));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex").join("config.toml"))
        .ok_or_else(|| "missing CODEX_HOME and HOME; cannot locate Codex config".to_string())
}
