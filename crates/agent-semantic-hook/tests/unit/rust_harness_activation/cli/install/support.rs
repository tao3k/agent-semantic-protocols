use crate::rust_harness_activation::support::{asp_command, temp_project_root};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn git_project_root(name: &str) -> PathBuf {
    let root = temp_project_root(name);
    std::fs::create_dir_all(root.join(".git")).expect("create temp git toplevel marker");
    root
}

pub(super) fn protocol_command() -> Command {
    let mut command = asp_command();
    command.env_remove("PRJ_CACHE_HOME");
    command
}

pub(super) fn assert_installed_hook_state(config: &toml::Value, config_path: &Path) {
    let state = config
        .get("hooks")
        .and_then(toml::Value::as_table)
        .and_then(|hooks| hooks.get("state"))
        .and_then(toml::Value::as_table)
        .expect("generated hook trust state");
    assert_eq!(state.len(), 8);
    let pre_tool_key = format!("{}:pre_tool_use:0:0", config_path.display());
    let pre_tool_hash = state
        .get(&pre_tool_key)
        .and_then(toml::Value::as_table)
        .and_then(|entry| entry.get("trusted_hash"))
        .and_then(toml::Value::as_str)
        .expect("pre tool use trusted hash");
    assert!(pre_tool_hash.starts_with("sha256:"));
}
