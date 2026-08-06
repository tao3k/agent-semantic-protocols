//! Immutable, process-independent Hook matcher snapshot publication and loading.

use agent_semantic_hook::{ClientHookConfig, DurableHookConfigArtifact, HookRuntime};
use memmap2::MmapOptions;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) struct LoadedHookConfig {
    pub(crate) config: Arc<ClientHookConfig>,
}

fn append_file_identity(hasher: &mut Sha256, path: &Path) -> Result<(), String> {
    hasher.update(path.as_os_str().as_encoded_bytes());
    match fs::read(path) {
        Ok(bytes) => {
            hasher.update([1]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => hasher.update([0]),
        Err(error) => {
            return Err(format!(
                "read Hook matcher generation input {}: {error}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn compiled_generation_key(config_path: &Path, project_root: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    hasher.update(b"asp-hook-compiled-mmap-generation-v1\0");
    hasher.update(project_root.as_os_str().as_encoded_bytes());
    hasher.update(agent_semantic_config::hook_client_contract_fingerprint().as_bytes());
    append_file_identity(&mut hasher, config_path)?;
    append_file_identity(
        &mut hasher,
        &agent_semantic_hook::project_agent_config_path(project_root),
    )?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn snapshot_path(project_root: &Path, generation: &str) -> Result<PathBuf, String> {
    Ok(agent_semantic_runtime::project_state_paths(project_root)?
        .hook_cache_dir
        .join("compiled-matchers")
        .join(format!("{generation}.v1.json")))
}

fn load_mmap_snapshot(path: &Path) -> Result<ClientHookConfig, String> {
    let file = File::open(path)
        .map_err(|error| format!("open Hook matcher snapshot {}: {error}", path.display()))?;
    if file
        .metadata()
        .map_err(|error| format!("stat Hook matcher snapshot {}: {error}", path.display()))?
        .len()
        == 0
    {
        return Err(format!(
            "Hook matcher snapshot is empty: {}",
            path.display()
        ));
    }
    // SAFETY: snapshots are published by writing a new file and atomically
    // renaming it into this content-addressed path. They are never mutated in
    // place, and this mapping is read-only for the lifetime of `file`.
    let mapped = unsafe { MmapOptions::new().map(&file) }
        .map_err(|error| format!("mmap Hook matcher snapshot {}: {error}", path.display()))?;
    let artifact: DurableHookConfigArtifact = serde_json::from_slice(&mapped)
        .map_err(|error| format!("decode Hook matcher snapshot {}: {error}", path.display()))?;
    ClientHookConfig::from_durable_snapshot_config(artifact)
        .map_err(|error| format!("hydrate Hook matcher snapshot {}: {error}", path.display()))
}

fn atomic_publish_snapshot(path: &Path, config: &ClientHookConfig) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("Hook matcher snapshot has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "create Hook matcher snapshot directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec(&config.durable_snapshot_config())
        .map_err(|error| format!("encode Hook matcher snapshot: {error}"))?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("compiled-hook-matcher"),
        std::process::id()
    ));
    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temp)
        .map_err(|error| format!("create Hook matcher snapshot {}: {error}", temp.display()))?;
    output
        .write_all(&bytes)
        .and_then(|()| output.sync_all())
        .map_err(|error| format!("write Hook matcher snapshot {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!(
            "publish Hook matcher snapshot {} -> {}: {error}",
            temp.display(),
            path.display()
        )
    })
}

pub(crate) fn load_fresh_hook_config(
    config_path: &Path,
    project_root: &Path,
) -> Result<(Arc<LoadedHookConfig>, &'static str), String> {
    let generation = compiled_generation_key(config_path, project_root)?;
    let snapshot = snapshot_path(project_root, &generation)?;
    if snapshot.is_file() {
        let config = load_mmap_snapshot(&snapshot)?;
        agent_semantic_hook::validate_match_policy_rule_coverage(&config).map_err(|error| {
            format!(
                "Hook matcher snapshot conformance failed for {}: {error}; run `asp hook install --client codex {}`",
                snapshot.display(),
                project_root.display()
            )
        })?;
        return Ok((
            Arc::new(LoadedHookConfig {
                config: Arc::new(config),
            }),
            "mmap-hit",
        ));
    }

    let config = agent_semantic_hook::load_client_config_for_project(config_path, project_root)
        .map_err(|error| {
            format!(
                "Hook matcher snapshot source is invalid for {}: {error}; run `asp hook install --client codex {}` to republish managed artifacts",
                config_path.display(),
                project_root.display()
            )
        })?;
    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    if config.contract_fingerprint() != Some(expected_fingerprint.as_str()) {
        return Err(format!(
            "Hook matcher snapshot source fingerprint mismatch for {}; run `asp hook install --client codex {}`",
            config_path.display(),
            project_root.display()
        ));
    }
    agent_semantic_hook::validate_match_policy_rule_coverage(&config).map_err(|error| {
        format!(
            "Hook matcher source conformance failed for {}: {error}; run `asp hook install --client codex {}`",
            config_path.display(),
            project_root.display()
        )
    })?;
    atomic_publish_snapshot(&snapshot, &config)?;
    Ok((
        Arc::new(LoadedHookConfig {
            config: Arc::new(config),
        }),
        "compiled-and-published",
    ))
}

pub(crate) fn apply_language_provider_projection(
    config: &ClientHookConfig,
    runtime: &mut HookRuntime,
    config_path: &Path,
) -> Result<(), String> {
    config
        .apply_language_provider_projection(runtime)
        .map_err(|error| {
            format!(
                "Hook language provider projection is invalid for {}: {error}",
                config_path.display()
            )
        })
}
