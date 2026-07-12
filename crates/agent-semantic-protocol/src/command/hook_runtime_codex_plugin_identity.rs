use std::env;
use std::fs;
use std::path::PathBuf;

const ASP_CODEX_PLUGIN_NAME: &str = "asp-codex-plugin";
const ASP_CODEX_PLUGIN_MARKETPLACE_NAME: &str = "asp-project";
const ASP_CODEX_PLUGIN_HOOKS_RELATIVE_PATH: &str = "hooks/hooks.json";

pub(super) fn codex_plugin_hook_key_source() -> String {
    format!(
        "{ASP_CODEX_PLUGIN_NAME}@{ASP_CODEX_PLUGIN_MARKETPLACE_NAME}:{ASP_CODEX_PLUGIN_HOOKS_RELATIVE_PATH}"
    )
}

pub(super) fn codex_global_plugin_hooks_json_path() -> Result<PathBuf, String> {
    let cache_root = codex_home_path()?
        .join("plugins")
        .join("cache")
        .join(ASP_CODEX_PLUGIN_MARKETPLACE_NAME)
        .join(ASP_CODEX_PLUGIN_NAME);
    let mut candidates = fs::read_dir(&cache_root)
        .map_err(|error| format!("failed to read {}: {error}", cache_root.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join(ASP_CODEX_PLUGIN_HOOKS_RELATIVE_PATH))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop().ok_or_else(|| {
        format!(
            "missing global Codex plugin hooks under {}",
            cache_root.display()
        )
    })
}

fn codex_home_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".codex"))
        .ok_or_else(|| "missing CODEX_HOME and HOME".to_string())
}
