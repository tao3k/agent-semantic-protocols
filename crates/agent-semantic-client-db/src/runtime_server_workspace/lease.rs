//! Immutable read lease for one resident workspace generation and overlay snapshot.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceMemoryBackend, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceProjectionLease,
};

#[derive(Debug, Clone)]
pub struct WorkspaceGenerationLease {
    pub(super) backend: Arc<WorkspaceMemoryBackend>,
    pub(super) overlay: super::resident_overlay::ResidentOverlaySnapshot,
}

impl WorkspaceGenerationLease {
    pub(crate) fn from_backend(backend: Arc<WorkspaceMemoryBackend>) -> Self {
        let overlays = Arc::new(super::resident_overlay::ResidentOverlayStore::new(Some(
            backend.generation(),
        )));
        let overlay = overlays.snapshot(backend.generation());
        Self { backend, overlay }
    }

    pub fn workspace_identity(&self) -> &str {
        &self.backend.generation().workspace_identity
    }

    pub fn epoch(&self) -> u64 {
        self.backend.generation().active_epoch
    }

    pub fn generation(&self) -> &WorkspaceMemoryGeneration {
        self.backend.generation()
    }

    pub fn project(&self, selector: &str) -> Option<WorkspaceProjectionLease> {
        self.backend.projection(selector)
    }

    pub fn owner(&self, owner_path: &str) -> Option<Arc<[u8]>> {
        self.overlay.owner_bytes(self.generation(), owner_path)
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: &str,
        structural_selector: &str,
    ) -> Result<super::WorkspaceRuntimeSelectorRead, String> {
        self.overlay
            .read_selector(self.generation(), projection_kind, structural_selector)
    }

    pub fn runtime_owner_snapshot(
        &self,
        owner_path: &str,
    ) -> Option<(String, WorkspaceOwnerSnapshot)> {
        self.overlay
            .owner_snapshot(self.generation(), owner_path)
            .map(|owner| (self.overlay.generation_digest().to_owned(), owner))
    }

    pub fn runtime_generation_digest(&self) -> String {
        self.overlay.generation_digest().to_owned()
    }

    pub fn read_source_index(
        &self,
        query: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        let generation = self.backend.generation();
        let positions = self
            .backend
            .source_index_owner_positions(query, limit as usize);
        let candidates = positions
            .into_iter()
            .map(|position| {
                let owner = &generation.owners[position];
                let text = std::str::from_utf8(&owner.bytes).unwrap_or_default();
                crate::ClientDbSourceIndexCandidate {
                    path: owner.owner_path.clone().into(),
                    language_id: language_id.cloned(),
                    provider_id: None,
                    source_kind: crate::ClientDbSourceIndexSourceKind::File,
                    line_count: Some(text.lines().count().max(1).min(u32::MAX as usize) as u32),
                    query_keys: crate::source_index::source_query_keys(&owner.owner_path, text)
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                    selector_symbol: None,
                    selector_kind: None,
                    selector_projection: None,
                }
            })
            .collect::<Vec<_>>();
        let state = if candidates.is_empty() {
            crate::ClientDbSourceIndexLookupState::Miss
        } else {
            crate::ClientDbSourceIndexLookupState::Hit
        };
        Ok(crate::ClientDbSourceIndexLookupResult {
            db_path: PathBuf::new(),
            state,
            candidates,
            source_snapshot: Some(generation.source_snapshot.clone()),
            index_artifact_digest: Some(crate::client_db_source_index_artifact_digest(
                &generation.source_snapshot,
            )),
        })
    }
}
