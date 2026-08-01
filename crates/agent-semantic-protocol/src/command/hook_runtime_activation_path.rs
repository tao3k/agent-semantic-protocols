use agent_semantic_hook::{default_activation_path, discover_activation_path};
use std::path::PathBuf;

pub(super) fn default_or_discovered_activation_path(payload: &serde_json::Value) -> PathBuf {
    let cwd = payload
        .get("cwd")
        .and_then(serde_json::Value::as_str)
        .filter(|cwd| !cwd.trim().is_empty())
        .map(PathBuf::from)
        .filter(|cwd| cwd.is_absolute())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    discover_activation_path(&cwd).unwrap_or_else(|| default_activation_path(&cwd))
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_activation_path.rs"]
mod tests;
