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

fn recovery_instruction() -> String {
    agent_semantic_runtime::resolve_state_home()
        .map(|state_home| {
            format!(
                "run a validated candidate `<candidate-asp> install binary --target {}` to republish the binary/config generation",
                state_home.join("runtime/bin/asp").display()
            )
        })
        .unwrap_or_else(|_| {
            "run a validated candidate `<candidate-asp> install binary --target <ASP_STATE_HOME>/runtime/bin/asp` to republish the binary/config generation".to_owned()
        })
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
    load_fresh_hook_config_inner(config_path, project_root, false)
}

fn load_fresh_hook_config_inner(
    config_path: &Path,
    project_root: &Path,
    managed_sync_attempted: bool,
) -> Result<(Arc<LoadedHookConfig>, &'static str), String> {
    let generation = compiled_generation_key(config_path, project_root)?;
    let snapshot = snapshot_path(project_root, &generation)?;
    let corrupt_snapshot = if snapshot.is_file() {
        match load_mmap_snapshot(&snapshot) {
            Ok(config) => {
                agent_semantic_hook::validate_match_policy_rule_coverage(&config).map_err(
                    |error| {
                        format!(
                            "Hook matcher snapshot conformance failed for {}: {error}; {}",
                            snapshot.display(),
                            recovery_instruction()
                        )
                    },
                )?;
                return Ok((
                    Arc::new(LoadedHookConfig {
                        config: Arc::new(config),
                    }),
                    "mmap-hit",
                ));
            }
            Err(_) => true,
        }
    } else {
        false
    };

    let config = agent_semantic_hook::load_client_config_for_project(config_path, project_root)
        .map_err(|error| {
            format!(
                "Hook matcher snapshot source is invalid for {}: {error}; {}",
                config_path.display(),
                recovery_instruction()
            )
        })?;
    let expected_fingerprint = agent_semantic_config::hook_client_contract_fingerprint();
    if config.contract_fingerprint() != Some(expected_fingerprint.as_str()) {
        let canonical_managed_config = agent_semantic_runtime::resolve_state_home()
            .ok()
            .map(|state_home| state_home.join("hooks/config.toml"));
        let is_canonical_managed_config = canonical_managed_config.as_deref().is_some_and(|path| {
            match (fs::canonicalize(path), fs::canonicalize(config_path)) {
                (Ok(canonical), Ok(actual)) => canonical == actual,
                _ => false,
            }
        });
        if !managed_sync_attempted && is_canonical_managed_config {
            crate::command::managed_hook_config::materialize(config_path).map_err(|error| {
                format!(
                    "Hook managed-config auto-sync failed for {}: {error}; {}",
                    config_path.display(),
                    recovery_instruction()
                )
            })?;
            let (loaded, _) = load_fresh_hook_config_inner(config_path, project_root, true)?;
            return Ok((loaded, "managed-config-auto-synced"));
        }
        return Err(format!(
            "Hook matcher snapshot source fingerprint mismatch for {}; {}",
            config_path.display(),
            recovery_instruction()
        ));
    }
    agent_semantic_hook::validate_match_policy_rule_coverage(&config).map_err(|error| {
        format!(
            "Hook matcher source conformance failed for {}: {error}; {}",
            config_path.display(),
            recovery_instruction()
        )
    })?;
    atomic_publish_snapshot(&snapshot, &config)?;
    Ok((
        Arc::new(LoadedHookConfig {
            config: Arc::new(config),
        }),
        if corrupt_snapshot {
            "compiled-and-published-after-corrupt-snapshot"
        } else {
            "compiled-and-published"
        },
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
