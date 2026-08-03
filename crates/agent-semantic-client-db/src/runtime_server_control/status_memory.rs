use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use memmap2::{Mmap, MmapMut, MmapOptions};
use tokio::sync::OnceCell;

use super::model::{
    RuntimeServerControlReceipt, RuntimeServerEndpoint, RuntimeServerState,
    RuntimeServerStatusSnapshot,
};

const STATUS_MEMORY_BYTES: u64 = 4096;
const GENERATION_OFFSET: usize = 0;
const PAYLOAD_LENGTH_OFFSET: usize = 8;
const PAYLOAD_OFFSET: usize = 16;

static STATUS_MEMORY_READERS: OnceCell<
    RwLock<HashMap<String, Arc<OnceCell<Arc<RuntimeServerStatusMemoryReader>>>>>,
> = OnceCell::const_new();

pub(crate) struct RuntimeServerStatusMemoryWriter {
    mapping: MmapMut,
    endpoint: RuntimeServerEndpoint,
    generation: u64,
    graph_turbo_resident:
        Option<std::sync::Arc<std::sync::RwLock<super::GraphTurboResidentStatus>>>,
}

struct RuntimeServerStatusMemoryReader {
    mapping: Mmap,
    cached_snapshot: RwLock<Option<(u64, Arc<RuntimeServerStatusSnapshot>)>>,
    snapshot_decode_count: AtomicU64,
    snapshot_cache_hit_count: AtomicU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeServerStatusMemoryMetrics {
    pub reader_open_count: u64,
    pub snapshot_decode_count: u64,
    pub snapshot_cache_hit_count: u64,
}

impl RuntimeServerStatusMemoryWriter {
    pub(crate) async fn create(endpoint: &RuntimeServerEndpoint) -> Result<Self, String> {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&endpoint.status_memory_path)
            .await
            .map_err(|error| format!("failed to create Runtime Server status memory: {error}"))?;
        file.set_len(STATUS_MEMORY_BYTES)
            .await
            .map_err(|error| format!("failed to size Runtime Server status memory: {error}"))?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|error| format!("failed to protect Runtime Server status memory: {error}"))?;
        let file = file.into_std().await;
        // SAFETY: the file is exclusively initialized by the elected Runtime Server,
        // has a stable non-zero length, and the mapping is retained for the writer lifetime.
        let mapping = unsafe { MmapOptions::new().map_mut(&file) }
            .map_err(|error| format!("failed to map Runtime Server status memory: {error}"))?;
        let mut writer = Self {
            mapping,
            endpoint: endpoint.clone(),
            generation: 0,
            graph_turbo_resident: None,
        };
        writer.publish(RuntimeServerState::Starting, 0)?;
        Ok(writer)
    }

    pub(crate) fn publish(
        &mut self,
        state: RuntimeServerState,
        workspace_entry_count: usize,
    ) -> Result<(), String> {
        let committed_generation = self.generation.saturating_add(2);
        let snapshot = RuntimeServerStatusSnapshot::new(
            committed_generation,
            state,
            &self.endpoint,
            workspace_entry_count,
        );
        let snapshot = RuntimeServerStatusSnapshot {
            graph_turbo_resident: self
                .graph_turbo_resident
                .as_ref()
                .and_then(|status| status.read().ok().map(|status| status.clone())),
            ..snapshot
        };
        let payload = serde_json::to_vec(&snapshot)
            .map_err(|error| format!("failed to encode Runtime Server status memory: {error}"))?;
        if payload.len() > self.mapping.len().saturating_sub(PAYLOAD_OFFSET) {
            return Err("Runtime Server status memory payload exceeds fixed mapping".to_owned());
        }
        generation(&self.mapping).store(committed_generation - 1, Ordering::Release);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_OFFSET].fill(0);
        self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
            .copy_from_slice(&(payload.len() as u32).to_le_bytes());
        self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + payload.len()].copy_from_slice(&payload);
        generation(&self.mapping).store(committed_generation, Ordering::Release);
        self.generation = committed_generation;
        Ok(())
    }

    pub(crate) fn set_graph_turbo_resident(
        &mut self,
        status: std::sync::Arc<std::sync::RwLock<super::GraphTurboResidentStatus>>,
    ) {
        self.graph_turbo_resident = Some(status);
    }
}

impl RuntimeServerStatusMemoryReader {
    async fn open(endpoint: &RuntimeServerEndpoint) -> Result<Self, String> {
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .open(&endpoint.status_memory_path)
            .await
            .map_err(|error| format!("failed to open Runtime Server status memory: {error}"))?;
        let file = file.into_std().await;
        // SAFETY: the elected Runtime Server owns the fixed-size backing file and
        // publishes payloads under the generation seqlock.
        let mapping = unsafe { MmapOptions::new().map(&file) }
            .map_err(|error| format!("failed to map Runtime Server status memory: {error}"))?;
        if mapping.len() != STATUS_MEMORY_BYTES as usize {
            return Err("Runtime Server status memory has an invalid size".to_owned());
        }
        Ok(Self {
            mapping,
            cached_snapshot: RwLock::new(None),
            snapshot_decode_count: AtomicU64::new(0),
            snapshot_cache_hit_count: AtomicU64::new(0),
        })
    }

    fn read(&self) -> Result<Arc<RuntimeServerStatusSnapshot>, String> {
        let before = generation(&self.mapping).load(Ordering::Acquire);
        let cached = || {
            self.cached_snapshot
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .map(|(_, snapshot)| Arc::clone(snapshot))
        };
        if before == 0 {
            return Err("Runtime Server status memory is not initialized".to_owned());
        }
        if before % 2 != 0 {
            return cached()
                .inspect(|_| {
                    self.snapshot_cache_hit_count
                        .fetch_add(1, Ordering::Relaxed);
                })
                .ok_or_else(|| {
                    "Runtime Server status publication is in progress before the first snapshot"
                        .to_owned()
                });
        }
        if let Some((cached_generation, snapshot)) = self
            .cached_snapshot
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            && *cached_generation == before
            && generation(&self.mapping).load(Ordering::Acquire) == before
        {
            self.snapshot_cache_hit_count
                .fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(snapshot));
        }
        let mut cached_snapshot = self
            .cached_snapshot
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((cached_generation, snapshot)) = cached_snapshot.as_ref()
            && *cached_generation == before
            && generation(&self.mapping).load(Ordering::Acquire) == before
        {
            self.snapshot_cache_hit_count
                .fetch_add(1, Ordering::Relaxed);
            return Ok(Arc::clone(snapshot));
        }
        let length = u32::from_le_bytes(
            self.mapping[PAYLOAD_LENGTH_OFFSET..PAYLOAD_LENGTH_OFFSET + 4]
                .try_into()
                .expect("fixed status payload length field"),
        ) as usize;
        if length == 0 || PAYLOAD_OFFSET + length > self.mapping.len() {
            return Err("Runtime Server status memory payload length is invalid".to_owned());
        }
        let payload = self.mapping[PAYLOAD_OFFSET..PAYLOAD_OFFSET + length].to_vec();
        let after = generation(&self.mapping).load(Ordering::Acquire);
        if before != after || after % 2 != 0 {
            return cached().ok_or_else(|| {
                "Runtime Server status publication changed during the first snapshot".to_owned()
            });
        }
        let snapshot: RuntimeServerStatusSnapshot = serde_json::from_slice(&payload)
            .map_err(|error| format!("failed to decode Runtime Server status memory: {error}"))?;
        snapshot.validate(after)?;
        let snapshot = Arc::new(snapshot);
        *cached_snapshot = Some((after, Arc::clone(&snapshot)));
        self.snapshot_decode_count.fetch_add(1, Ordering::Relaxed);
        Ok(snapshot)
    }

    fn metrics(&self) -> RuntimeServerStatusMemoryMetrics {
        RuntimeServerStatusMemoryMetrics {
            reader_open_count: 1,
            snapshot_decode_count: self.snapshot_decode_count.load(Ordering::Relaxed),
            snapshot_cache_hit_count: self.snapshot_cache_hit_count.load(Ordering::Relaxed),
        }
    }
}

pub fn runtime_server_status_memory_metrics(
    endpoint: &RuntimeServerEndpoint,
) -> Option<RuntimeServerStatusMemoryMetrics> {
    let readers = STATUS_MEMORY_READERS.get()?;
    let reader_slot = readers
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&endpoint.status_memory_path)
        .cloned()?;
    reader_slot.get().map(|reader| reader.metrics())
}

async fn runtime_server_status_memory_reader(
    endpoint: &RuntimeServerEndpoint,
) -> Result<Arc<RuntimeServerStatusMemoryReader>, String> {
    let readers = STATUS_MEMORY_READERS
        .get_or_init(|| async { RwLock::new(HashMap::new()) })
        .await;
    let reader_slot = {
        let cached = readers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cached.get(&endpoint.status_memory_path).cloned()
    };
    let reader_slot = match reader_slot {
        Some(reader_slot) => reader_slot,
        None => {
            let mut cached = readers
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Arc::clone(
                cached
                    .entry(endpoint.status_memory_path.clone())
                    .or_insert_with(|| Arc::new(OnceCell::const_new())),
            )
        }
    };
    let reader = reader_slot
        .get_or_try_init(|| async {
            RuntimeServerStatusMemoryReader::open(endpoint)
                .await
                .map(Arc::new)
        })
        .await?;
    Ok(Arc::clone(reader))
}

pub async fn prewarm_runtime_server_status_memory(
    endpoint: &RuntimeServerEndpoint,
) -> Result<RuntimeServerStatusMemoryMetrics, String> {
    let reader = runtime_server_status_memory_reader(endpoint).await?;
    reader.read()?;
    Ok(reader.metrics())
}

pub(crate) async fn read_runtime_server_status(
    endpoint: &RuntimeServerEndpoint,
    request_id: String,
) -> Result<RuntimeServerControlReceipt, String> {
    let reader = runtime_server_status_memory_reader(endpoint).await?;
    let snapshot = reader.read()?;
    snapshot.receipt(request_id, endpoint)
}

fn generation(mapping: &[u8]) -> &AtomicU64 {
    let pointer = mapping[GENERATION_OFFSET..].as_ptr().cast::<AtomicU64>();
    // SAFETY: mmap bases are page-aligned, GENERATION_OFFSET is aligned for
    // AtomicU64, and the mapping outlives the returned reference.
    unsafe { &*pointer }
}
