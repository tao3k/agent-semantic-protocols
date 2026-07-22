use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static PUBLISH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ManagedHookConfigStatus {
    Current,
    Created,
    Refreshed,
}

impl ManagedHookConfigStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Created => "created",
            Self::Refreshed => "refreshed-contract",
        }
    }
}

/// Materialize the complete fingerprint-bound template as one managed artifact.
/// User overrides belong in the typed project overlay, never in this generated base.
pub(super) fn materialize(path: &Path) -> Result<ManagedHookConfigStatus, String> {
    let expected = agent_semantic_hook::default_client_config_template();
    match std::fs::read(path) {
        Ok(current) if current == expected.as_bytes() => Ok(ManagedHookConfigStatus::Current),
        Ok(_) => {
            atomic_publish(path, expected.as_bytes())?;
            verify(path, expected.as_bytes())?;
            Ok(ManagedHookConfigStatus::Refreshed)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            atomic_publish(path, expected.as_bytes())?;
            verify(path, expected.as_bytes())?;
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
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "managed hook config content identity mismatch for {}",
            path.display()
        ))
    }
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
