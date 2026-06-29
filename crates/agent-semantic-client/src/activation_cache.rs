//! SQLite-backed provider activation selection guard.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use agent_semantic_client_core::{ASP_PROVIDER_ACTIVATION_PATH_ENV, ProviderRegistrySnapshot};
use agent_semantic_client_db::{ClientDbEngine, ClientDbProviderCommandSelection};
use agent_semantic_hook::{
    HookRuntime, ProviderCommandSelection, build_default_activation, discover_activation_path,
    load_or_sync_activation, project_agent_config_path, provider_command_selections,
    write_activation,
};
use agent_semantic_runtime::is_project_activation_path;
use sha2::{Digest, Sha256};

const PROVIDER_COMMAND_SELECTION_FINGERPRINT_VERSION: &str = "provider-command-selection.v1";

pub(crate) fn load_provider_registry_snapshot(
    activation_root: &Path,
    project_root: &Path,
    emit_stderr_diagnostics: bool,
) -> Result<ProviderRegistrySnapshot, String> {
    ensure_generated_activation_provider_commands_current(project_root, emit_stderr_diagnostics)?;
    ProviderRegistrySnapshot::load(activation_root)
}

fn ensure_generated_activation_provider_commands_current(
    project_root: &Path,
    emit_stderr_diagnostics: bool,
) -> Result<(), String> {
    if activation_refresh_disabled() {
        return Ok(());
    }
    let Some(activation_path) = active_generated_activation_path(project_root) else {
        return Ok(());
    };
    if !activation_path.is_file() {
        return Ok(());
    }
    let runtime = load_or_sync_activation(&activation_path, project_root)?;
    let context_fingerprint = provider_command_selection_context_fingerprint(project_root)?;
    let db_engine = ClientDbEngine::resolve(project_root)?;
    let mut db = db_engine.open_or_create()?;
    if let Some(cached) =
        db.lookup_provider_command_selections(project_root, &context_fingerprint)?
        && cached.iter().all(cached_provider_executable_is_fresh)
    {
        if runtime_matches_cached_provider_commands(&runtime, &cached) {
            return Ok(());
        }
        sync_activation_from_current_selection(
            project_root,
            &activation_path,
            emit_stderr_diagnostics,
        )?;
        return Ok(());
    }

    let current = provider_command_selections(project_root)?;
    let current_rows = current
        .iter()
        .map(provider_command_selection_row)
        .collect::<Vec<_>>();
    db.replace_provider_command_selections(project_root, &context_fingerprint, &current_rows)?;
    if !runtime_matches_provider_commands(&runtime, &current) {
        sync_activation_from_current_selection(
            project_root,
            &activation_path,
            emit_stderr_diagnostics,
        )?;
    }
    Ok(())
}

fn activation_refresh_disabled() -> bool {
    env::var("ASP_PROVIDER_ACTIVATION_REFRESH")
        .is_ok_and(|value| matches!(value.as_str(), "0" | "false" | "off"))
}

fn active_generated_activation_path(project_root: &Path) -> Option<PathBuf> {
    let activation_path = env::var_os(ASP_PROVIDER_ACTIVATION_PATH_ENV)
        .map(PathBuf::from)
        .or_else(|| discover_activation_path(project_root));
    activation_path.filter(|path| is_generated_activation_path(path))
}

fn is_generated_activation_path(path: &Path) -> bool {
    is_project_activation_path(path)
}

fn sync_activation_from_current_selection(
    project_root: &Path,
    activation_path: &Path,
    emit_stderr_diagnostics: bool,
) -> Result<(), String> {
    if emit_stderr_diagnostics {
        eprintln!(
            "[agent-semantic-client] syncing generated activation {}: provider command selection changed",
            activation_path.display()
        );
    }
    let activation = build_default_activation(project_root)?;
    write_activation(activation_path, &activation)
}

fn provider_command_selection_context_fingerprint(project_root: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(PROVIDER_COMMAND_SELECTION_FINGERPRINT_VERSION.as_bytes());
    hasher.update(b"\0PATH\0");
    hasher.update(
        env::var_os("PATH")
            .unwrap_or_default()
            .to_string_lossy()
            .as_bytes(),
    );
    let config_path = project_agent_config_path(project_root);
    hasher.update(b"\0.agents/asp.toml\0");
    match fs::read(&config_path) {
        Ok(bytes) => hasher.update(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update(b"<missing>"),
        Err(error) => {
            return Err(format!(
                "failed to read provider activation cache context {}: {error}",
                config_path.display()
            ));
        }
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn runtime_matches_cached_provider_commands(
    runtime: &HookRuntime,
    cached: &[ClientDbProviderCommandSelection],
) -> bool {
    runtime.providers.len() == cached.len()
        && runtime.providers.iter().all(|provider| {
            cached.iter().any(|selection| {
                selection.manifest_id() == provider.manifest_id
                    && selection.manifest_digest() == provider.manifest_digest
                    && selection.language_id() == provider.language_id
                    && selection.provider_id() == provider.provider_id
                    && selection.binary() == provider.binary
                    && selection.execution() == provider.execution.as_str()
                    && selection.provider_command_prefix() == provider.provider_command_prefix
            })
        })
}

fn runtime_matches_provider_commands(
    runtime: &HookRuntime,
    current: &[ProviderCommandSelection],
) -> bool {
    runtime.providers.len() == current.len()
        && runtime.providers.iter().all(|provider| {
            current.iter().any(|selection| {
                selection.manifest_id == provider.manifest_id
                    && selection.manifest_digest == provider.manifest_digest
                    && selection.language_id == provider.language_id
                    && selection.provider_id == provider.provider_id
                    && selection.binary == provider.binary
                    && selection.execution == provider.execution
                    && selection.provider_command_prefix == provider.provider_command_prefix
            })
        })
}

fn provider_command_selection_row(
    selection: &ProviderCommandSelection,
) -> ClientDbProviderCommandSelection {
    let executable = selection
        .provider_command_prefix
        .first()
        .and_then(|path| executable_metadata(path));
    ClientDbProviderCommandSelection::new(
        selection.manifest_id.clone(),
        selection.manifest_digest.clone(),
        selection.language_id.clone(),
        selection.provider_id.clone(),
        selection.binary.clone(),
        selection.execution.as_str().to_string(),
        selection.provider_command_prefix.clone(),
        executable.as_ref().map(|metadata| metadata.path.clone()),
        executable.as_ref().map(|metadata| metadata.len),
        executable.as_ref().and_then(|metadata| metadata.mtime_ms),
    )
}

fn cached_provider_executable_is_fresh(selection: &ClientDbProviderCommandSelection) -> bool {
    let Some(path) = selection.executable_path() else {
        return true;
    };
    executable_metadata(path).is_some_and(|metadata| {
        Some(metadata.len) == selection.executable_len()
            && metadata.mtime_ms == selection.executable_mtime_ms()
    })
}

struct ExecutableMetadata {
    path: String,
    len: i64,
    mtime_ms: Option<i64>,
}

fn executable_metadata(path: &str) -> Option<ExecutableMetadata> {
    let metadata = fs::metadata(path).ok()?;
    Some(ExecutableMetadata {
        path: path.to_string(),
        len: metadata.len().min(i64::MAX as u64) as i64,
        mtime_ms: metadata.modified().ok().and_then(|modified| {
            modified
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        }),
    })
}
