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

pub(super) async fn load_activation_for_language(
    path: &Path,
    invocation_root: &Path,
    language_id: &str,
) -> Result<HookRuntime, String> {
    let _ = path;
    let session =
        crate::server::runtime_server::runtime_server_workspace_session_async(invocation_root)
            .await?;
    let runtime = session
        .resolve_provider_runtime(language_id.into())
        .await
        .and_then(|runtime| {
            serde_json::from_value(runtime)
                .map_err(|error| format!("decode Runtime-owned provider runtime: {error}"))
        });
    runtime.map_err(|error| {
        format!(
                "state=cold-required reasonKind=runtime-server-provider-runtime-unavailable languageId={language_id} projectRoot={} error={error}",
            invocation_root.display(),
        )
    })
}

/// Resolve a provider runtime inside the resident ASP Server.
///
/// Server-owned Tokio tasks call this pure catalog boundary directly. They
/// must never route back through the external Runtime IPC client and deadlock
/// or re-enter workspace admission.
pub(crate) fn resolve_provider_runtime_in_server(
    project_root: &Path,
    language_id: &str,
) -> Result<HookRuntime, String> {
    let activation_path = provider_activation_path(project_root);
    agent_semantic_hook::registered_language_runtime(project_root, language_id, &activation_path)
}
