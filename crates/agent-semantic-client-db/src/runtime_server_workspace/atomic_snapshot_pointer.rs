// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use memmap2::{Mmap, MmapMut, MmapOptions};
use serde::Serialize;
use serde::de::DeserializeOwned;

const POINTER_LEN: usize = 4_096;
const PAYLOAD_OFFSET: usize = 16;
const MAX_READ_ATTEMPTS: usize = 64;

#[derive(Debug, Clone)]
pub(super) struct AtomicSnapshotPointerWriter {
    path: PathBuf,
    context: &'static str,
    file: Arc<tokio::fs::File>,
    mapping: Arc<std::sync::Mutex<MmapMut>>,
}

impl AtomicSnapshotPointerWriter {
    pub(super) async fn open(path: PathBuf, context: &'static str) -> Result<Self, String> {
        let reset = match tokio::fs::metadata(&path).await {
            Ok(metadata) => metadata.len() != POINTER_LEN as u64,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => {
                return Err(format!(
                    "inspect {context} pointer `{}`: {error}",
                    path.display()
                ));
            }
        };
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(reset)
            .read(true)
            .write(true)
            .open(&path)
            .await
            .map_err(|error| format!("open {context} pointer `{}`: {error}", path.display()))?;
        file.set_len(POINTER_LEN as u64)
            .await
            .map_err(|error| format!("size {context} pointer `{}`: {error}", path.display()))?;
        let sync_file = file
            .try_clone()
            .await
            .map_err(|error| format!("clone {context} pointer `{}`: {error}", path.display()))?;
        let file = file.into_std().await;
        let pointer_path = path.clone();
        let mapping = tokio::task::spawn_blocking(move || unsafe {
            // SAFETY: the file is read/write, fixed-size, and remains immutable in
            // length while the mapping exists.
            MmapOptions::new().len(POINTER_LEN).map_mut(&file)
        })
        .await
        .map_err(|error| format!("map {context} pointer task failed: {error}"))?
        .map_err(|error| {
            format!(
                "map {context} pointer `{}`: {error}",
                pointer_path.display()
            )
        })?;
        set_private_permissions(&path).await?;
        Ok(Self {
            path,
            context,
            file: Arc::new(sync_file),
            mapping: Arc::new(std::sync::Mutex::new(mapping)),
        })
    }

    pub(super) async fn publish<T: Serialize>(&self, snapshot: &T) -> Result<(), String> {
        let payload = serde_json::to_vec(snapshot)
            .map_err(|error| format!("encode {} pointer: {error}", self.context))?;
        if payload.len() > POINTER_LEN - PAYLOAD_OFFSET {
            return Err(format!(
                "{} pointer payload exceeds capacity: actual={} capacity={}",
                self.context,
                payload.len(),
                POINTER_LEN - PAYLOAD_OFFSET
            ));
        }
        let mapping = Arc::clone(&self.mapping);
        let path = self.path.clone();
        let context = self.context;
        tokio::task::spawn_blocking(move || {
            let mut mapping = mapping
                .lock()
                .map_err(|_| format!("{context} pointer mapping lock is poisoned"))?;
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
            mapping
                .flush_async()
                .map_err(|error| format!("flush {context} pointer `{}`: {error}", path.display()))
        })
        .await
        .map_err(|error| format!("publish {context} pointer task failed: {error}"))??;
        self.file.sync_data().await.map_err(|error| {
            format!(
                "sync {} pointer `{}`: {error}",
                self.context,
                self.path.display()
            )
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug)]
pub(super) struct AtomicSnapshotPointerReader<T> {
    mapping: Mmap,
    context: &'static str,
    leased_snapshot: tokio::sync::watch::Sender<Option<PointerLease<T>>>,
    decode_count: AtomicU64,
    marker: PhantomData<T>,
}

#[derive(Debug, Clone)]
struct PointerLease<T> {
    generation: u64,
    snapshot: T,
}

impl<T> AtomicSnapshotPointerReader<T>
where
    T: Clone + DeserializeOwned,
{
    pub(super) async fn open(path: &Path, context: &'static str) -> Result<Self, String> {
        Self::open_optional(path, context).await?.ok_or_else(|| {
            format!(
                "open {context} pointer `{}`: file not found",
                path.display()
            )
        })
    }

    pub(super) async fn open_optional(
        path: &Path,
        context: &'static str,
    ) -> Result<Option<Self>, String> {
        let file = match tokio::fs::File::open(path).await {
            Ok(file) => file.into_std().await,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(format!(
                    "open {context} pointer `{}`: {error}",
                    path.display()
                ));
            }
        };
        let mapping = unsafe {
            // SAFETY: the writer fixes the file length and never replaces the
            // inode; publication only mutates the mapped payload under seqlock.
            // mmap installs a lazy virtual-memory mapping and does not read the
            // pointer payload, so a blocking-pool hop would only add scheduler
            // latency to cold session admission.
            MmapOptions::new().len(POINTER_LEN).map(&file)
        }
        .map_err(|error| format!("map {context} pointer `{}`: {error}", path.display()))?;
        let (leased_snapshot, _) = tokio::sync::watch::channel(None);
        Ok(Some(Self {
            mapping,
            context,
            leased_snapshot,
            decode_count: AtomicU64::new(0),
            marker: PhantomData,
        }))
    }

    pub(super) fn read(&self) -> Result<T, String> {
        if let Some(snapshot) = self.cached_snapshot() {
            return Ok(snapshot);
        }
        match decode_consistent_snapshot::<T>(&self.mapping, self.context) {
            Ok(Some((generation, snapshot))) => {
                self.decode_count.fetch_add(1, Ordering::Relaxed);
                Ok(self.publish_lease(generation, snapshot))
            }
            Ok(None) => Err(format!(
                "{} pointer has no complete generation",
                self.context
            )),
            Err(ReadSnapshotError::Unstable) => self
                .leased_snapshot
                .borrow()
                .as_ref()
                .map(|lease| lease.snapshot.clone())
                .ok_or_else(|| {
                    format!("{} pointer has no complete generation lease", self.context)
                }),
            Err(ReadSnapshotError::Invalid(error)) => Err(error),
        }
    }

    pub(super) fn read_optional(&self) -> Result<Option<T>, String> {
        if let Some(snapshot) = self.cached_snapshot() {
            return Ok(Some(snapshot));
        }
        match decode_consistent_snapshot::<T>(&self.mapping, self.context) {
            Ok(Some((generation, snapshot))) => {
                self.decode_count.fetch_add(1, Ordering::Relaxed);
                Ok(Some(self.publish_lease(generation, snapshot)))
            }
            Ok(None) => Ok(None),
            Err(ReadSnapshotError::Unstable) => Ok(self
                .leased_snapshot
                .borrow()
                .as_ref()
                .map(|lease| lease.snapshot.clone())),
            Err(ReadSnapshotError::Invalid(error)) => Err(error),
        }
    }

    pub(super) fn read_previous_valid_optional(&self) -> Option<T> {
        if let Some(snapshot) = self.cached_snapshot() {
            return Some(snapshot);
        }
        match decode_consistent_snapshot::<T>(&self.mapping, self.context) {
            Ok(Some((generation, snapshot))) => {
                self.decode_count.fetch_add(1, Ordering::Relaxed);
                Some(self.publish_lease(generation, snapshot))
            }
            Ok(None) | Err(ReadSnapshotError::Invalid(_)) => None,
            Err(ReadSnapshotError::Unstable) => self
                .leased_snapshot
                .borrow()
                .as_ref()
                .map(|lease| lease.snapshot.clone()),
        }
    }

    fn cached_snapshot(&self) -> Option<T> {
        let observed = pointer_generation(&self.mapping).load(Ordering::Acquire);
        if observed == 0 || observed & 1 == 1 {
            return None;
        }
        self.leased_snapshot
            .borrow()
            .as_ref()
            .filter(|lease| lease.generation == observed)
            .map(|lease| lease.snapshot.clone())
    }

    fn publish_lease(&self, generation: u64, snapshot: T) -> T {
        let mut snapshot = Some(snapshot);
        self.leased_snapshot.send_if_modified(|lease| {
            if lease
                .as_ref()
                .is_some_and(|current| current.generation >= generation)
            {
                return false;
            }
            *lease = Some(PointerLease {
                generation,
                snapshot: snapshot.take().expect("candidate lease is consumed once"),
            });
            true
        });
        self.leased_snapshot
            .borrow()
            .as_ref()
            .expect("decoded pointer publishes a complete lease")
            .snapshot
            .clone()
    }
}

#[derive(Debug)]
enum ReadSnapshotError {
    Unstable,
    Invalid(String),
}

fn decode_consistent_snapshot<T: DeserializeOwned>(
    mapping: &[u8],
    context: &str,
) -> Result<Option<(u64, T)>, ReadSnapshotError> {
    for _ in 0..MAX_READ_ATTEMPTS {
        let before = pointer_generation(mapping).load(Ordering::Acquire);
        if before == 0 {
            return Ok(None);
        }
        if before & 1 == 1 {
            std::hint::spin_loop();
            continue;
        }
        let payload_len =
            u64::from_ne_bytes(mapping[8..PAYLOAD_OFFSET].try_into().map_err(|_| {
                ReadSnapshotError::Invalid(format!("{context} pointer length header is invalid"))
            })?) as usize;
        if payload_len == 0 || payload_len > POINTER_LEN - PAYLOAD_OFFSET {
            return Err(ReadSnapshotError::Invalid(format!(
                "{context} pointer payload length is invalid: {payload_len}"
            )));
        }
        let decoded =
            serde_json::from_slice(&mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload_len]);
        let after = pointer_generation(mapping).load(Ordering::Acquire);
        if before != after || after & 1 == 1 {
            std::hint::spin_loop();
            continue;
        }
        return decoded
            .map(|snapshot| Some((after, snapshot)))
            .map_err(|error| {
                ReadSnapshotError::Invalid(format!("decode {context} pointer: {error}"))
            });
    }
    Err(ReadSnapshotError::Unstable)
}

fn pointer_generation(mapping: &[u8]) -> &AtomicU64 {
    assert!(mapping.len() >= PAYLOAD_OFFSET);
    // SAFETY: mmap base addresses are page-aligned. The first eight bytes are
    // exclusively reserved for the cross-process atomic generation word.
    unsafe { &*mapping.as_ptr().cast::<AtomicU64>() }
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .map_err(|error| format!("protect atomic pointer `{}`: {error}", path.display()))
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}
