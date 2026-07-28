use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    HookRuntime, default_activation_path, discover_activation_path, language_activation_path,
    load_or_sync_activation, load_or_sync_activation_for_language, parse_hook_activation,
};

pub(super) fn load_activation_for_language_message() -> Option<HookRuntime> {
    let cwd = env::current_dir().ok()?;
    let activation_path = provider_activation_path(&cwd);
    let text = fs::read_to_string(activation_path).ok()?;
    parse_hook_activation(&text).ok()
}

pub(super) fn provider_activation_path(invocation_root: &Path) -> PathBuf {
    discover_activation_path(invocation_root)
        .unwrap_or_else(|| default_activation_path(invocation_root))
}

pub(super) fn load_activation(path: &Path, invocation_root: &Path) -> Result<HookRuntime, String> {
    load_or_sync_activation(path, invocation_root)
}

pub(super) fn load_activation_for_language(
    path: &Path,
    invocation_root: &Path,
    language_id: &str,
) -> Result<HookRuntime, String> {
    load_or_sync_activation_for_language(path, invocation_root, language_id)
}

pub(super) fn activation_path_for_language(
    path: &Path,
    invocation_root: &Path,
    language_id: &str,
) -> PathBuf {
    language_activation_path(path, invocation_root, language_id)
}
