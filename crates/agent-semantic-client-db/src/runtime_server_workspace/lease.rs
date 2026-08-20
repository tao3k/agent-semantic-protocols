//! Immutable read lease for one resident workspace generation and overlay snapshot.

use std::path::PathBuf;
use std::sync::Arc;

use super::{
    WorkspaceMemoryBackend, WorkspaceMemoryGeneration, WorkspaceOwnerSnapshot,
    WorkspaceProjectionLease,
};

#[derive(Debug)]
pub(crate) struct WorkspaceResidentActivity {
    accepting: std::sync::atomic::AtomicBool,
    in_flight_requests: std::sync::atomic::AtomicUsize,
    live_leases: std::sync::atomic::AtomicUsize,
    activity_origin: tokio::time::Instant,
    last_activity_micros: std::sync::atomic::AtomicI64,
}

impl WorkspaceResidentActivity {
    pub(crate) fn new() -> Self {
        Self {
            accepting: std::sync::atomic::AtomicBool::new(true),
            in_flight_requests: std::sync::atomic::AtomicUsize::new(0),
            live_leases: std::sync::atomic::AtomicUsize::new(0),
            activity_origin: tokio::time::Instant::now(),
            last_activity_micros: std::sync::atomic::AtomicI64::new(0),
        }
    }

    fn touch(&self) {
        self.last_activity_micros.store(
            self.activity_elapsed_micros(),
            std::sync::atomic::Ordering::Release,
        );
    }

    fn activity_elapsed_micros(&self) -> i64 {
        i64::try_from(self.activity_origin.elapsed().as_micros()).unwrap_or(i64::MAX)
    }

    pub(crate) fn begin_request(self: &Arc<Self>) -> Result<WorkspaceResidentRequestGuard, String> {
        use std::sync::atomic::Ordering;

        if !self.accepting.load(Ordering::Acquire) {
            return Err("resident workspace admission is closed".to_owned());
        }
        self.in_flight_requests.fetch_add(1, Ordering::AcqRel);
        if !self.accepting.load(Ordering::Acquire) {
            self.in_flight_requests.fetch_sub(1, Ordering::AcqRel);
            return Err("resident workspace admission closed during request entry".to_owned());
        }
        self.touch();
        Ok(WorkspaceResidentRequestGuard {
            activity: Arc::clone(self),
        })
    }

    fn acquire_lease(self: &Arc<Self>) -> Result<(), String> {
        use std::sync::atomic::Ordering;

        if !self.accepting.load(Ordering::Acquire) {
            return Err("resident workspace admission is closed".to_owned());
        }
        self.live_leases.fetch_add(1, Ordering::AcqRel);
        if !self.accepting.load(Ordering::Acquire) {
            self.live_leases.fetch_sub(1, Ordering::AcqRel);
            return Err("resident workspace admission closed during lease acquisition".to_owned());
        }
        self.touch();
        Ok(())
    }

    fn clone_lease(&self) {
        self.live_leases
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        self.touch();
    }

    fn release_lease(&self) {
        self.live_leases
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        self.touch();
    }

    pub(crate) fn try_close_for_retirement(
        &self,
        workspace_path_exists: bool,
        idle_timeout: std::time::Duration,
    ) -> bool {
        use std::sync::atomic::Ordering;

        let last_activity_micros = self.last_activity_micros.load(Ordering::Acquire);
        let idle_elapsed = std::time::Duration::from_micros(
            u64::try_from(
                self.activity_elapsed_micros()
                    .saturating_sub(last_activity_micros),
            )
            .unwrap_or(0),
        );
        if workspace_path_exists && idle_elapsed < idle_timeout {
            return false;
        }
        if self.live_leases.load(Ordering::Acquire) != 0
            || self.in_flight_requests.load(Ordering::Acquire) != 0
        {
            return false;
        }
        if self
            .accepting
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        if self.live_leases.load(Ordering::Acquire) == 0
            && self.in_flight_requests.load(Ordering::Acquire) == 0
        {
            true
        } else {
            self.accepting.store(true, Ordering::Release);
            false
        }
    }

    pub(crate) fn reopen(&self) {
        self.accepting
            .store(true, std::sync::atomic::Ordering::Release);
        self.touch();
    }

    #[cfg(test)]
    pub(crate) fn set_idle_for_test(&self, idle_for: std::time::Duration) {
        let idle_micros = i64::try_from(idle_for.as_micros()).unwrap_or(i64::MAX);
        self.last_activity_micros.store(
            self.activity_elapsed_micros().saturating_sub(idle_micros),
            std::sync::atomic::Ordering::Release,
        );
    }

    pub(crate) fn counts(&self) -> (usize, usize) {
        use std::sync::atomic::Ordering;
        (
            self.live_leases.load(Ordering::Acquire),
            self.in_flight_requests.load(Ordering::Acquire),
        )
    }
}

#[derive(Debug)]
pub(crate) struct WorkspaceResidentRequestGuard {
    activity: Arc<WorkspaceResidentActivity>,
}

impl Drop for WorkspaceResidentRequestGuard {
    fn drop(&mut self) {
        self.activity
            .in_flight_requests
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
        self.activity.touch();
    }
}

#[derive(Debug)]
pub struct WorkspaceGenerationLease {
    pub(super) backend: Arc<WorkspaceMemoryBackend>,
    pub(super) overlay: super::resident_overlay::ResidentOverlaySnapshot,
    pub(super) activity: Option<Arc<WorkspaceResidentActivity>>,
}

impl Clone for WorkspaceGenerationLease {
    fn clone(&self) -> Self {
        if let Some(activity) = &self.activity {
            activity.clone_lease();
        }
        Self {
            backend: Arc::clone(&self.backend),
            overlay: self.overlay.clone(),
            activity: self.activity.clone(),
        }
    }
}

impl Drop for WorkspaceGenerationLease {
    fn drop(&mut self) {
        if let Some(activity) = &self.activity {
            activity.release_lease();
        }
    }
}

impl WorkspaceGenerationLease {
    pub(crate) fn from_backend(backend: Arc<WorkspaceMemoryBackend>) -> Self {
        let overlays = Arc::new(super::resident_overlay::ResidentOverlayStore::new(Some(
            backend.generation(),
        )));
        let overlay = overlays.snapshot(backend.generation());
        Self {
            backend,
            overlay,
            activity: None,
        }
    }

    pub(crate) fn from_resident(
        backend: Arc<WorkspaceMemoryBackend>,
        overlay: super::resident_overlay::ResidentOverlaySnapshot,
        activity: Arc<WorkspaceResidentActivity>,
    ) -> Result<Self, String> {
        activity.acquire_lease()?;
        Ok(Self {
            backend,
            overlay,
            activity: Some(activity),
        })
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
        projection_kind: super::ExactProjectionKind,
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

    pub fn relations_from(
        &self,
        endpoint_kind: &str,
        endpoint_id: &str,
    ) -> Vec<
        &agent_semantic_content_identity::provider_projection_relation::ProviderProjectedRelation,
    > {
        self.backend.relations_from(endpoint_kind, endpoint_id)
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
        let state = if candidates.is_empty()
            && generation.workspace_generation.owner_count == 0
            && generation.workspace_generation.leaf_count == 0
        {
            crate::ClientDbSourceIndexLookupState::ColdRequired
        } else if candidates.is_empty() {
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
