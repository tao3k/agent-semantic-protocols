// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Resident client lease for the immutable exact-projection mmap.

use std::path::Path;

use tokio::fs;

use super::{MappedWorkspaceExactProjection, decode_header};
use crate::runtime_server_workspace::{
    WorkspaceGenerationPointerReader, WorkspaceOwnerSearchSnapshot, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeSelectorRead,
};

#[derive(Clone, Debug)]
pub struct WorkspaceExactProjectionDataPlaneClient {
    inner: std::sync::Arc<WorkspaceExactProjectionDataPlaneClientInner>,
}

#[derive(Debug)]
struct WorkspaceExactProjectionDataPlaneClientInner {
    pointer: Option<WorkspaceGenerationPointerReader>,
    current: parking_lot::RwLock<std::sync::Arc<MappedWorkspaceExactProjection>>,
}

#[derive(Debug, Default)]
struct WorkspaceExactProjectionDataPlaneCell {
    client: tokio::sync::OnceCell<WorkspaceExactProjectionDataPlaneClient>,
}

static EXACT_PROJECTION_DATA_PLANE_CELLS: std::sync::OnceLock<
    parking_lot::RwLock<
        std::collections::BTreeMap<
            std::path::PathBuf,
            std::sync::Arc<WorkspaceExactProjectionDataPlaneCell>,
        >,
    >,
> = std::sync::OnceLock::new();

fn exact_projection_data_plane_cell(
    pointer_path: &Path,
) -> std::sync::Arc<WorkspaceExactProjectionDataPlaneCell> {
    let cells = EXACT_PROJECTION_DATA_PLANE_CELLS.get_or_init(Default::default);
    if let Some(cell) = cells.read().get(pointer_path) {
        return std::sync::Arc::clone(cell);
    }
    let mut cells = cells.write();
    std::sync::Arc::clone(
        cells.entry(pointer_path.to_path_buf()).or_insert_with(|| {
            std::sync::Arc::new(WorkspaceExactProjectionDataPlaneCell::default())
        }),
    )
}

#[derive(Debug)]
pub enum WorkspaceExactProjectionDataPlaneOpen {
    Ready(WorkspaceExactProjectionDataPlaneClient),
    Missing,
    RecoveryRequired { reason: String },
}

impl WorkspaceExactProjectionDataPlaneClient {
    pub async fn open_state(
        pointer_path: &Path,
    ) -> Result<WorkspaceExactProjectionDataPlaneOpen, String> {
        if !fs::try_exists(pointer_path)
            .await
            .map_err(|error| format!("inspect workspace generation pointer: {error}"))?
        {
            return Ok(WorkspaceExactProjectionDataPlaneOpen::Missing);
        }
        match Self::open(pointer_path).await {
            Ok(client) => Ok(WorkspaceExactProjectionDataPlaneOpen::Ready(client)),
            Err(reason) => Ok(WorkspaceExactProjectionDataPlaneOpen::RecoveryRequired { reason }),
        }
    }

    pub(crate) fn invalidate_committed_pointer(pointer_path: &Path) {
        if let Some(cells) = EXACT_PROJECTION_DATA_PLANE_CELLS.get() {
            cells.write().remove(pointer_path);
        }
    }

    pub(crate) async fn prime_committed_pointer(pointer_path: &Path) -> Result<(), String> {
        Self::invalidate_committed_pointer(pointer_path);
        Self::open(pointer_path).await.map(|_| ())
    }

    pub async fn open(pointer_path: &Path) -> Result<Self, String> {
        let cell = exact_projection_data_plane_cell(pointer_path);
        let mut client = cell
            .client
            .get_or_try_init(|| async {
                let pointer = WorkspaceGenerationPointerReader::open(pointer_path).await?;
                let snapshot = pointer.read()?;
                snapshot.validate()?;
                let mapped = MappedWorkspaceExactProjection::open(&snapshot).await?;
                Ok::<_, String>(Self {
                    inner: std::sync::Arc::new(WorkspaceExactProjectionDataPlaneClientInner {
                        pointer: Some(pointer),
                        current: parking_lot::RwLock::new(std::sync::Arc::new(mapped)),
                    }),
                })
            })
            .await?
            .clone();
        client.refresh_if_changed().await?;
        Ok(client)
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: super::super::model::ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        self.inner
            .current
            .read()
            .read_runtime_selector(projection_kind, structural_selector)
    }

    /// Resolve a projection evidence context from the same immutable mmap.
    pub fn projection_evidence_context(
        &self,
        evidence_context_ref: &str,
    ) -> Result<Option<Vec<u8>>, String> {
        self.inner
            .current
            .read()
            .projection_evidence_context(evidence_context_ref)
    }

    /// Return the resident content identity for one exact owner without
    /// opening the workspace database or contacting the control plane.
    pub fn owner_content_digest(&self, owner_path: &str) -> Result<Option<String>, String> {
        self.inner.current.read().owner_content_digest(owner_path)
    }

    pub fn contains_owner(&self, owner: &WorkspaceOwnerSnapshot) -> Result<bool, String> {
        self.inner.current.read().contains_owner(owner)
    }

    /// Resolve one owner directly from the immutable exact-generation index.
    pub fn owner_snapshot(
        &self,
        owner_path: &str,
    ) -> Result<Option<WorkspaceOwnerSnapshot>, String> {
        let mapped = self.inner.current.read();
        let Some((owner_index, owner)) = mapped.find_owner(owner_path)? else {
            return Ok(None);
        };
        mapped.owner_snapshot(owner_index, &owner).map(Some)
    }

    /// Read only committed selector seeds without copying owner payloads.
    pub fn owner_search_snapshot(
        &self,
        owner_path: &str,
        query_terms: &[String],
        limit: usize,
    ) -> Result<Option<WorkspaceOwnerSearchSnapshot>, String> {
        let mapped = self.inner.current.read();
        let Some((owner_index, owner)) = mapped.find_owner(owner_path)? else {
            return Ok(None);
        };
        let (candidate_count, selectors) =
            mapped.owner_search_seeds(owner_index, query_terms, limit)?;
        Ok(Some(WorkspaceOwnerSearchSnapshot {
            owner_path: mapped.owner_path(&owner)?.to_owned(),
            content_digest: mapped.owner_digest(&owner)?.to_owned(),
            candidate_count,
            selectors,
        }))
    }

    #[must_use]
    pub fn generation_digest(&self) -> String {
        self.inner.current.read().generation_digest.clone()
    }

    #[must_use]
    pub fn root_digest(&self) -> String {
        self.inner.current.read().root_digest.clone()
    }

    pub async fn refresh_if_changed(&mut self) -> Result<bool, String> {
        let Some(pointer) = &self.inner.pointer else {
            return Ok(false);
        };
        let snapshot = pointer.read()?;
        snapshot.validate()?;
        let current = self.inner.current.read();
        decode_header(&current.mapping)?;
        if current.epoch == snapshot.active_epoch {
            return Ok(false);
        }
        drop(current);
        let mapped = MappedWorkspaceExactProjection::open(&snapshot).await?;
        *self.inner.current.write() = std::sync::Arc::new(mapped);
        Ok(true)
    }
}
