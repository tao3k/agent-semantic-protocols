use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

static PUBLISH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ManagedHookConfigStatus {
    Current,
    Created,
    Migrated,
}

impl ManagedHookConfigStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Created => "created",
            Self::Migrated => "migrated-managed",
        }
    }
}

/// Materialize the complete fingerprint-bound template as one managed artifact.
/// User overrides belong in the typed project overlay, never in this generated base.
pub(super) fn materialize(path: &Path) -> Result<ManagedHookConfigStatus, String> {
    let expected = agent_semantic_hook::default_client_config_template();
    let expected_bytes = expected.as_bytes();
    match std::fs::read(path) {
        Ok(current) if current == expected_bytes => {
            if !sidecar_matches(path, expected_bytes)? {
                publish_sidecar(path, expected_bytes)?;
            }
            verify(path, expected_bytes)?;
            Ok(ManagedHookConfigStatus::Current)
        }
        Ok(current) => {
            if !sidecar_matches(path, &current)? {
                return Err(format!(
                    "user-config-contract-unproven: managed hook config {} differs from template and has no matching ownership sidecar",
                    path.display()
                ));
            }
            atomic_publish(path, expected_bytes)?;
            publish_sidecar(path, expected_bytes)?;
            verify(path, expected_bytes)?;
            Ok(ManagedHookConfigStatus::Migrated)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            atomic_publish(path, expected_bytes)?;
            publish_sidecar(path, expected_bytes)?;
            verify(path, expected_bytes)?;
            Ok(ManagedHookConfigStatus::Created)
        }
        Err(error) => Err(format!(
            "failed to read managed hook config {}: {error}",
            path.display()
        )),
    }
}

fn verify(path: &Path, expected: &[u8]) -> Result<(), String> {
    let actual = std::fs::read(path).map_err(|error| {
        format!(
            "failed to verify managed hook config {}: {error}",
            path.display()
        )
    })?;
    if actual == expected && sidecar_matches(path, expected)? {
        Ok(())
    } else {
        Err(format!(
            "managed hook config content identity mismatch for {}",
            path.display()
        ))
    }
}

fn sidecar_path(path: &Path) -> Result<std::path::PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("managed hook config path has no parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    Ok(parent.join(format!("{file_name}.managed.sha256")))
}

fn publish_sidecar(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_publish(&sidecar_path(path)?, digest_hex(bytes).as_bytes())
}

fn sidecar_matches(path: &Path, bytes: &[u8]) -> Result<bool, String> {
    let sidecar = sidecar_path(path)?;
    match std::fs::read_to_string(&sidecar) {
        Ok(current) => Ok(current.trim() == digest_hex(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "failed to read managed hook config sidecar {}: {error}",
            sidecar.display()
        )),
    }
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn atomic_publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("managed hook config path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create managed hook config directory {}: {error}",
            parent.display()
        )
    })?;
    let sequence = PUBLISH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    let publish = (|| -> Result<(), String> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "failed to create managed hook config publish file {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(bytes).map_err(|error| {
            format!(
                "failed to write managed hook config publish file {}: {error}",
                temporary.display()
            )
        })?;
        file.sync_all().map_err(|error| {
            format!(
                "failed to sync managed hook config publish file {}: {error}",
                temporary.display()
            )
        })?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|error| {
            format!(
                "failed to atomically publish managed hook config {}: {error}",
                path.display()
            )
        })
    })();
    if publish.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    publish
}

#[cfg(test)]
#[path = "../../tests/unit/managed_hook_config.rs"]
mod tests;
