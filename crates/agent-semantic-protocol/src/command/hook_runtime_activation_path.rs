use agent_semantic_hook::{default_activation_path, discover_activation_path};
use std::path::{Path, PathBuf};

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

fn activation_relative_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let configured = PathBuf::from(project_root);
    let root = if configured.is_absolute() {
        configured
    } else {
        activation_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(configured)
    };
    std::fs::canonicalize(&root).unwrap_or(root)
}

pub(super) fn hook_runtime_project_root(activation_path: &Path, project_root: &str) -> PathBuf {
    let activation_root = activation_relative_project_root(activation_path, project_root);
    if activation_root_is_global_hook_state(activation_path, &activation_root) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        return std::fs::canonicalize(&cwd).unwrap_or(cwd);
    }
    activation_root
}

fn activation_root_is_global_hook_state(activation_path: &Path, activation_root: &Path) -> bool {
    let Some(activation_dir) = activation_path.parent() else {
        return false;
    };
    if std::fs::canonicalize(activation_dir).unwrap_or_else(|_| activation_dir.to_path_buf())
        != std::fs::canonicalize(activation_root).unwrap_or_else(|_| activation_root.to_path_buf())
    {
        return false;
    }
    activation_dir.file_name().and_then(|name| name.to_str()) == Some("state")
        && activation_dir.ancestors().any(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "hooks")
        })
}

#[cfg(test)]
#[path = "../../tests/unit/command/hook_activation_path.rs"]
mod tests;
