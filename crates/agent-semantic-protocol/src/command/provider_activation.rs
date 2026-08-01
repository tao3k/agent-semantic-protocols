use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use agent_semantic_hook::{
    HookRuntime, default_activation_path, discover_activation_path,
    load_activation as load_published_activation, parse_hook_activation,
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
    load_published_activation(path).map_err(|error| {
        format!(
            "state=cold-required reasonKind=published-activation-required projectRoot={} activation={} error={error}",
            invocation_root.display(),
            path.display()
        )
    })
}

pub(super) fn load_activation_for_language(
    path: &Path,
    invocation_root: &Path,
    language_id: &str,
) -> Result<HookRuntime, String> {
    agent_semantic_hook::registered_language_runtime(invocation_root, language_id, path).map_err(
        |error| {
        format!(
                "state=cold-required reasonKind=registered-language-runtime-unavailable languageId={language_id} projectRoot={} error={error}",
            invocation_root.display(),
        )
        },
    )
}
