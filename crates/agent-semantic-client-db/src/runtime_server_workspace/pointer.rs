use super::model::WorkspaceGenerationSnapshot;
use memmap2::{Mmap, MmapMut, MmapOptions};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering, fence},
    },
};
use tokio::{
    fs::{self, OpenOptions},
    sync::Mutex,
};

const POINTER_LEN: usize = 4_096;
const PAYLOAD_OFFSET: usize = 16;
const MAX_READ_ATTEMPTS: usize = 64;

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceGenerationPointerWriter {
    path: PathBuf,
    mapping: Arc<Mutex<MmapMut>>,
}

impl WorkspaceGenerationPointerWriter {
    pub(crate) async fn open(directory: &Path) -> Result<Self, String> {
        let path = directory.join("active-generation.pointer");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .map_err(|error| format!("open workspace generation pointer: {error}"))?;
        if file
            .metadata()
            .await
            .map_err(|error| format!("inspect workspace generation pointer: {error}"))?
            .len()
            != POINTER_LEN as u64
        {
            file.set_len(POINTER_LEN as u64)
                .await
                .map_err(|error| format!("size workspace generation pointer: {error}"))?;
        }
        set_private_permissions(&path).await?;
        let file = file.into_std().await;
        let mapping = unsafe {
            // SAFETY: the file has a fixed non-zero length, remains owned by the
            // mapping, and publication is serialized by the workspace writer lane.
            MmapOptions::new()
                .len(POINTER_LEN)
                .map_mut(&file)
                .map_err(|error| format!("map workspace generation pointer: {error}"))?
        };
        Ok(Self {
            path,
            mapping: Arc::new(Mutex::new(mapping)),
        })
    }

    pub(crate) async fn publish(
        &self,
        snapshot: &WorkspaceGenerationSnapshot,
    ) -> Result<(), String> {
        snapshot.validate()?;
        let payload = serde_json::to_vec(snapshot)
            .map_err(|error| format!("encode workspace generation pointer: {error}"))?;
        let payload_end = PAYLOAD_OFFSET
            .checked_add(payload.len())
            .ok_or_else(|| "workspace generation pointer payload length overflows".to_owned())?;
        if payload_end > POINTER_LEN {
            return Err("workspace generation pointer payload exceeds fixed mapping".to_owned());
        }

        let mut mapping = self.mapping.lock().await;
        let current = pointer_generation(&mapping).load(Ordering::Acquire);
        let writing = current.wrapping_add(1) | 1;
        pointer_generation(&mapping).store(writing, Ordering::Release);
        fence(Ordering::Release);
        mapping[8..16].copy_from_slice(
            &u64::try_from(payload.len())
                .map_err(|_| "workspace generation pointer payload is too large".to_owned())?
                .to_le_bytes(),
        );
        mapping[PAYLOAD_OFFSET..payload_end].copy_from_slice(&payload);
        mapping[payload_end..].fill(0);
        fence(Ordering::Release);
        pointer_generation(&mapping).store(writing.wrapping_add(1), Ordering::Release);
        mapping
            .flush_async()
            .map_err(|error| format!("flush workspace generation pointer: {error}"))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub struct WorkspaceGenerationPointerReader {
    mapping: Mmap,
}

impl WorkspaceGenerationPointerReader {
    pub async fn open(path: &Path) -> Result<Self, String> {
        let file = fs::File::open(path)
            .await
            .map_err(|error| format!("open workspace generation pointer reader: {error}"))?;
        if file
            .metadata()
            .await
            .map_err(|error| format!("inspect workspace generation pointer reader: {error}"))?
            .len()
            != POINTER_LEN as u64
        {
            return Err("workspace generation pointer has an invalid fixed length".to_owned());
        }
        let file = file.into_std().await;
        let mapping = unsafe {
            // SAFETY: the writer fixes the file length before publication and
            // readers only access the immutable payload guarded by the seqlock.
            MmapOptions::new()
                .len(POINTER_LEN)
                .map(&file)
                .map_err(|error| format!("map workspace generation pointer reader: {error}"))?
        };
        Ok(Self { mapping })
    }

    pub fn read(&self) -> Result<WorkspaceGenerationSnapshot, String> {
        for _ in 0..MAX_READ_ATTEMPTS {
            let before = pointer_generation(&self.mapping).load(Ordering::Acquire);
            if before == 0 || before & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let payload_len =
                usize::try_from(u64::from_le_bytes(self.mapping[8..16].try_into().map_err(
                    |_| "workspace generation pointer length is invalid".to_owned(),
                )?))
                .map_err(|_| "workspace generation pointer length overflows usize".to_owned())?;
            let payload_end = PAYLOAD_OFFSET
                .checked_add(payload_len)
                .ok_or_else(|| "workspace generation pointer length overflows".to_owned())?;
            if payload_end > self.mapping.len() {
                return Err("workspace generation pointer payload is truncated".to_owned());
            }
            let payload = self.mapping[PAYLOAD_OFFSET..payload_end].to_vec();
            fence(Ordering::Acquire);
            let after = pointer_generation(&self.mapping).load(Ordering::Acquire);
            if before != after || after & 1 == 1 {
                std::hint::spin_loop();
                continue;
            }
            let snapshot = serde_json::from_slice::<WorkspaceGenerationSnapshot>(&payload)
                .map_err(|error| format!("decode workspace generation pointer: {error}"))?;
            snapshot.validate()?;
            return Ok(snapshot);
        }
        Err("workspace generation pointer remained unstable after bounded retries".to_owned())
    }
}

fn pointer_generation(mapping: &[u8]) -> &AtomicU64 {
    let pointer = mapping.as_ptr().cast::<AtomicU64>();
    unsafe {
        // SAFETY: mmap base addresses are page-aligned and the mapping is at
        // least eight bytes long. Atomic access is confined to the first word.
        &*pointer
    }
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|error| format!("set workspace generation pointer permissions: {error}"))
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
