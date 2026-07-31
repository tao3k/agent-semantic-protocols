use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use memmap2::{Mmap, MmapMut, MmapOptions};
use tokio::sync::Mutex;

use super::WorkspaceGenerationSnapshot;

const POINTER_LEN: usize = 4_096;
const PAYLOAD_OFFSET: usize = 16;
const MAX_READ_ATTEMPTS: usize = 64;
const POINTER_FILE_NAME: &str = "active-generation.pointer";

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceGenerationPointerWriter {
    path: PathBuf,
    mapping: Arc<Mutex<MmapMut>>,
}

impl WorkspaceGenerationPointerWriter {
    pub(crate) async fn open(directory: &Path) -> Result<Self, String> {
        tokio::fs::create_dir_all(directory)
            .await
            .map_err(|error| {
                format!(
                    "failed to create workspace generation directory `{}`: {error}",
                    directory.display()
                )
            })?;
        let path = directory.join(POINTER_FILE_NAME);
        let mapping_path = path.clone();
        let mapping = tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&mapping_path)
                .map_err(|error| {
                    format!(
                        "failed to open workspace generation pointer `{}`: {error}",
                        mapping_path.display()
                    )
                })?;
            file.set_len(POINTER_LEN as u64).map_err(|error| {
                format!(
                    "failed to size workspace generation pointer `{}`: {error}",
                    mapping_path.display()
                )
            })?;
            // SAFETY: the file is opened read/write, is held for the mapping
            // operation, and has been sized to the exact mapped length.
            unsafe { MmapOptions::new().len(POINTER_LEN).map_mut(&file) }.map_err(|error| {
                format!(
                    "failed to map workspace generation pointer `{}`: {error}",
                    mapping_path.display()
                )
            })
        })
        .await
        .map_err(|error| format!("workspace generation pointer open task failed: {error}"))??;
        set_private_permissions(&path).await?;
        Ok(Self {
            path,
            mapping: Arc::new(Mutex::new(mapping)),
        })
    }

    pub(crate) async fn publish(
        &self,
        snapshot: &WorkspaceGenerationSnapshot,
    ) -> Result<(), String> {
        let payload = serde_json::to_vec(snapshot)
            .map_err(|error| format!("failed to encode workspace generation pointer: {error}"))?;
        if payload.len() > POINTER_LEN - PAYLOAD_OFFSET {
            return Err(format!(
                "workspace generation pointer payload exceeds capacity: actual={} capacity={}",
                payload.len(),
                POINTER_LEN - PAYLOAD_OFFSET
            ));
        }

        let mut mapping = self.mapping.lock().await;
        let current = pointer_generation(&mapping).load(Ordering::Acquire);
        let writing = if current & 1 == 0 {
            current.wrapping_add(1)
        } else {
            current.wrapping_add(2)
        };
        pointer_generation(&mapping).store(writing, Ordering::Release);
        mapping[8..PAYLOAD_OFFSET].copy_from_slice(&(payload.len() as u64).to_ne_bytes());
        mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload.len()].copy_from_slice(&payload);
        mapping[PAYLOAD_OFFSET + payload.len()..].fill(0);
        pointer_generation(&mapping).store(writing.wrapping_add(1), Ordering::Release);
        mapping.flush_async().map_err(|error| {
            format!(
                "failed to flush workspace generation pointer `{}`: {error}",
                self.path.display()
            )
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub struct WorkspaceGenerationPointerReader {
    mapping: Mmap,
    leased_snapshot: RwLock<Option<WorkspaceGenerationSnapshot>>,
}

impl WorkspaceGenerationPointerReader {
    pub async fn open(path: &Path) -> Result<Self, String> {
        let mapping_path = path.to_path_buf();
        let mapping = tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new()
                .read(true)
                .open(&mapping_path)
                .map_err(|error| {
                    format!(
                        "failed to open workspace generation pointer `{}`: {error}",
                        mapping_path.display()
                    )
                })?;
            // SAFETY: the read-only file remains valid for the duration of the
            // mapping operation and the writer fixes its length at POINTER_LEN.
            unsafe { MmapOptions::new().len(POINTER_LEN).map(&file) }.map_err(|error| {
                format!(
                    "failed to map workspace generation pointer `{}`: {error}",
                    mapping_path.display()
                )
            })
        })
        .await
        .map_err(|error| format!("workspace generation pointer open task failed: {error}"))??;
        Ok(Self {
            mapping,
            leased_snapshot: RwLock::new(None),
        })
    }

    pub fn read(&self) -> Result<WorkspaceGenerationSnapshot, String> {
        match decode_consistent_snapshot(&self.mapping) {
            Ok(snapshot) => {
                let mut lease = self
                    .leased_snapshot
                    .write()
                    .map_err(|_| "workspace generation pointer lease is poisoned".to_owned())?;
                *lease = Some(snapshot.clone());
                Ok(snapshot)
            }
            Err(ReadSnapshotError::Unstable) => self
                .leased_snapshot
                .read()
                .map_err(|_| "workspace generation pointer lease is poisoned".to_owned())?
                .clone()
                .ok_or_else(|| {
                    "workspace generation pointer has no complete generation lease".to_owned()
                }),
            Err(ReadSnapshotError::Invalid(error)) => Err(error),
        }
    }
}

#[derive(Debug)]
enum ReadSnapshotError {
    Unstable,
    Invalid(String),
}

fn decode_consistent_snapshot(
    mapping: &[u8],
) -> Result<WorkspaceGenerationSnapshot, ReadSnapshotError> {
    for _ in 0..MAX_READ_ATTEMPTS {
        let before = pointer_generation(mapping).load(Ordering::Acquire);
        if before == 0 || before & 1 == 1 {
            std::hint::spin_loop();
            continue;
        }
        let payload_len =
            u64::from_ne_bytes(mapping[8..PAYLOAD_OFFSET].try_into().map_err(|_| {
                ReadSnapshotError::Invalid(
                    "workspace generation pointer length header is invalid".to_owned(),
                )
            })?) as usize;
        if payload_len == 0 || payload_len > POINTER_LEN - PAYLOAD_OFFSET {
            return Err(ReadSnapshotError::Invalid(format!(
                "workspace generation pointer payload length is invalid: {payload_len}"
            )));
        }
        let payload = mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload_len].to_vec();
        let after = pointer_generation(mapping).load(Ordering::Acquire);
        if before != after || after & 1 == 1 {
            std::hint::spin_loop();
            continue;
        }
        return serde_json::from_slice(&payload).map_err(|error| {
            ReadSnapshotError::Invalid(format!(
                "failed to decode workspace generation pointer: {error}"
            ))
        });
    }
    Err(ReadSnapshotError::Unstable)
}

fn pointer_generation(mapping: &[u8]) -> &AtomicU64 {
    assert!(mapping.len() >= PAYLOAD_OFFSET);
    // SAFETY: mmap base addresses are page-aligned and therefore aligned for
    // AtomicU64. The pointer file is fixed-size and the first eight bytes are
    // reserved exclusively for this atomic generation word.
    unsafe { &*mapping.as_ptr().cast::<AtomicU64>() }
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|error| {
            format!(
                "failed to protect workspace generation pointer `{}`: {error}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
