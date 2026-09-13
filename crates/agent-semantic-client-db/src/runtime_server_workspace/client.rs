// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{
    WorkspaceMemoryBackend, lease::WorkspaceGenerationLease,
    pointer::WorkspaceGenerationPointerReader, segment::MappedWorkspaceGeneration,
};
use parking_lot::RwLock;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Debug)]
pub struct WorkspaceGenerationDataPlaneClient {
    inner: Arc<WorkspaceGenerationDataPlaneClientInner>,
}

#[derive(Debug)]
struct WorkspaceGenerationDataPlaneClientInner {
    pointer_path: PathBuf,
    pointer: WorkspaceGenerationPointerReader,
    current: RwLock<Arc<WorkspaceMemoryBackend>>,
}

#[derive(Debug, Default)]
struct WorkspaceGenerationDataPlaneCell {
    client: tokio::sync::OnceCell<WorkspaceGenerationDataPlaneClient>,
    cold_open_count: AtomicU64,
    warm_hit_count: AtomicU64,
    coalesced_open_count: AtomicU64,
    refresh_open_count: AtomicU64,
}

static DATA_PLANE_CELLS: OnceLock<
    RwLock<BTreeMap<PathBuf, Arc<WorkspaceGenerationDataPlaneCell>>>,
> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkspaceGenerationDataPlaneCacheReceipt {
    pub cold_open_count: u64,
    pub warm_hit_count: u64,
    pub coalesced_open_count: u64,
    pub refresh_open_count: u64,
}

#[derive(Debug)]
pub enum WorkspaceGenerationDataPlaneOpen {
    Ready(WorkspaceGenerationDataPlaneClient),
    Missing,
    RecoveryRequired { reason: String },
}

impl WorkspaceGenerationDataPlaneClient {
    pub async fn open_state(
        pointer_path: &Path,
    ) -> Result<WorkspaceGenerationDataPlaneOpen, String> {
        if let Some(client) = cached_client(pointer_path) {
            let cell = data_plane_cell(pointer_path);
            cell.warm_hit_count.fetch_add(1, Ordering::Relaxed);
            return match client.refresh_if_changed_with_cell(&cell).await {
                Ok(_) => Ok(WorkspaceGenerationDataPlaneOpen::Ready(client)),
                Err(reason) => Ok(WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason }),
            };
        }
        if !tokio::fs::try_exists(pointer_path)
            .await
            .map_err(|error| format!("inspect workspace generation pointer: {error}"))?
        {
            return Ok(WorkspaceGenerationDataPlaneOpen::Missing);
        }
        match Self::open(pointer_path).await {
            Ok(client) => Ok(WorkspaceGenerationDataPlaneOpen::Ready(client)),
            Err(reason) => Ok(WorkspaceGenerationDataPlaneOpen::RecoveryRequired { reason }),
        }
    }

    pub async fn open(pointer_path: &Path) -> Result<Self, String> {
        let cell = data_plane_cell(pointer_path);
        if let Some(client) = cell.client.get() {
            cell.warm_hit_count.fetch_add(1, Ordering::Relaxed);
            client.refresh_if_changed_with_cell(&cell).await?;
            return Ok(client.clone());
        }
        if cell.cold_open_count.load(Ordering::Acquire) > 0 {
            cell.coalesced_open_count.fetch_add(1, Ordering::Relaxed);
        }
        cell.client
            .get_or_try_init(|| async {
                cell.cold_open_count.fetch_add(1, Ordering::Release);
                Self::open_uncached(pointer_path).await
            })
            .await
            .cloned()
    }

    async fn open_uncached(pointer_path: &Path) -> Result<Self, String> {
        let pointer = WorkspaceGenerationPointerReader::open(pointer_path).await?;
        let snapshot = pointer.read()?;
        snapshot.validate()?;
        let mapped =
            MappedWorkspaceGeneration::open(Path::new(&snapshot.mmap_segment_path)).await?;
        let backend = mapped.backend();
        validate_pointer_generation(&snapshot, &backend)?;
        Ok(Self {
            inner: Arc::new(WorkspaceGenerationDataPlaneClientInner {
                pointer_path: pointer_path.to_path_buf(),
                pointer,
                current: RwLock::new(backend),
            }),
        })
    }

    pub fn lease(&self) -> WorkspaceGenerationLease {
        WorkspaceGenerationLease::from_backend(Arc::clone(&self.inner.current.read()))
    }

    pub async fn refresh_if_changed(&self) -> Result<bool, String> {
        let cell = data_plane_cell(&self.inner.pointer_path);
        self.refresh_if_changed_with_cell(&cell).await
    }

    async fn refresh_if_changed_with_cell(
        &self,
        cell: &WorkspaceGenerationDataPlaneCell,
    ) -> Result<bool, String> {
        let snapshot = self.inner.pointer.read()?;
        snapshot.validate()?;
        if self.inner.current.read().generation().active_epoch == snapshot.active_epoch {
            return Ok(false);
        }
        cell.refresh_open_count.fetch_add(1, Ordering::Relaxed);
        let mapped =
            MappedWorkspaceGeneration::open(Path::new(&snapshot.mmap_segment_path)).await?;
        let backend = mapped.backend();
        validate_pointer_generation(&snapshot, &backend)?;
        *self.inner.current.write() = backend;
        Ok(true)
    }

    pub fn cache_receipt(pointer_path: &Path) -> WorkspaceGenerationDataPlaneCacheReceipt {
        let Some(cell) = DATA_PLANE_CELLS
            .get()
            .and_then(|cells| cells.read().get(pointer_path).cloned())
        else {
            return WorkspaceGenerationDataPlaneCacheReceipt::default();
        };
        WorkspaceGenerationDataPlaneCacheReceipt {
            cold_open_count: cell.cold_open_count.load(Ordering::Relaxed),
            warm_hit_count: cell.warm_hit_count.load(Ordering::Relaxed),
            coalesced_open_count: cell.coalesced_open_count.load(Ordering::Relaxed),
            refresh_open_count: cell.refresh_open_count.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn invalidate_committed_pointer(pointer_path: &Path) {
        if let Some(cells) = DATA_PLANE_CELLS.get() {
            cells.write().remove(pointer_path);
        }
    }
}

fn data_plane_cell(pointer_path: &Path) -> Arc<WorkspaceGenerationDataPlaneCell> {
    let cells = DATA_PLANE_CELLS.get_or_init(Default::default);
    if let Some(cell) = cells.read().get(pointer_path) {
        return Arc::clone(cell);
    }
    let mut cells = cells.write();
    Arc::clone(
        cells
            .entry(pointer_path.to_path_buf())
            .or_insert_with(|| Arc::new(WorkspaceGenerationDataPlaneCell::default())),
    )
}

fn cached_client(pointer_path: &Path) -> Option<WorkspaceGenerationDataPlaneClient> {
    DATA_PLANE_CELLS
        .get()
        .and_then(|cells| cells.read().get(pointer_path).cloned())
        .and_then(|cell| cell.client.get().cloned())
}

fn validate_pointer_generation(
    snapshot: &super::model::WorkspaceGenerationSnapshot,
    backend: &WorkspaceMemoryBackend,
) -> Result<(), String> {
    let generation = backend.generation();
    if generation.workspace_identity != snapshot.workspace_identity
        || generation.active_epoch != snapshot.active_epoch
        || generation.generation_digest != snapshot.generation_digest
        || generation.memory_backend_digest != snapshot.memory_backend_digest
        || generation.root_depth != snapshot.root_depth
    {
        return Err(format!(
            "workspace generation pointer does not match mapped generation: workspaceIdentity={} epoch={}",
            snapshot.workspace_identity, snapshot.active_epoch
        ));
    }
    Ok(())
}
