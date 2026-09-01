use std::path::Path;

use crate::runtime_server_opentelemetry::{
    RuntimePerformanceObservation, try_record_to_active_runtime,
};
use crate::runtime_server_workspace::{
    ExactProjectionKind, WorkspaceExactProjectionDataPlaneClient, WorkspaceOwnerSnapshot,
    WorkspaceRuntimeMerkleOwnerRead, WorkspaceRuntimeSelectorRead,
    WorkspaceSearchGenerationDataPlaneClient,
};

/// A process-local, immutable view of one published Runtime generation.
///
/// Opening the view belongs to admission/setup. Every read method is
/// synchronous and cannot perform socket, filesystem, database, provider, or
/// scheduler work.
pub struct RuntimeResidentReadClient {
    exact_projection: WorkspaceExactProjectionDataPlaneClient,
    search_projection: WorkspaceSearchGenerationDataPlaneClient,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeResidentReadWorkCounters {
    pub provider_process_count: u64,
    pub scheduler_task_count: u64,
    pub filesystem_read_count: u64,
    pub database_read_count: u64,
    pub socket_operation_count: u64,
}

impl RuntimeResidentReadClient {
    pub async fn open(pointer_path: &Path, project_root: &Path) -> Result<Self, String> {
        Ok(Self {
            exact_projection: WorkspaceExactProjectionDataPlaneClient::open(pointer_path).await?,
            search_projection: WorkspaceSearchGenerationDataPlaneClient::open(
                pointer_path,
                project_root,
            )
            .await?,
        })
    }

    pub fn read_runtime_selector(
        &self,
        projection_kind: ExactProjectionKind,
        structural_selector: &str,
    ) -> Result<WorkspaceRuntimeSelectorRead, String> {
        self.exact_projection
            .read_runtime_selector(projection_kind, structural_selector)
    }

    pub fn owner_snapshot(
        &self,
        owner_path: &str,
    ) -> Result<Option<WorkspaceOwnerSnapshot>, String> {
        self.exact_projection.owner_snapshot(owner_path)
    }

    pub fn read_runtime_owner(
        &self,
        owner_path: &str,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead, String> {
        let generation_digest = self.generation_digest();
        let root_digest = self.root_digest();
        Ok(match self.owner_snapshot(owner_path)? {
            Some(owner) => crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::Owner {
                generation_digest,
                root_digest,
                owner,
            },
            None => crate::runtime_server_workspace::WorkspaceRuntimeOwnerRead::OwnerMissing {
                generation_digest,
                root_digest,
            },
        })
    }

    /// Read bounded owner-search inputs without copying source or projections.
    pub fn read_runtime_owner_search(
        &self,
        owner_path: &str,
        query_terms: &[String],
        limit: usize,
    ) -> Result<crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead, String> {
        let generation_digest = self.generation_digest();
        let root_digest = self.root_digest();
        Ok(
            match self
                .exact_projection
                .owner_search_snapshot(owner_path, query_terms, limit)?
            {
                Some(owner) => {
                    crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::Owner {
                        generation_digest,
                        root_digest,
                        owner,
                    }
                }
                None => {
                    crate::runtime_server_workspace::WorkspaceRuntimeOwnerSearchRead::OwnerMissing {
                        generation_digest,
                        root_digest,
                    }
                }
            },
        )
    }

    pub fn read_merkle_owner(
        &self,
        owner_path: &str,
    ) -> Result<WorkspaceRuntimeMerkleOwnerRead, String> {
        self.search_projection.read_merkle_owner(owner_path)
    }

    pub fn read_source_index(
        &self,
        query: &str,
        authority: Option<&agent_semantic_search::ResidentSearchAuthority>,
        limit: u32,
    ) -> Result<agent_semantic_search_projection::ResidentSearchReadyResult, String> {
        self.search_projection
            .read_source_index(query, authority, limit)
    }

    pub fn parser_owned_callable_selector_pairs(
        &self,
        owner_paths: &[String],
    ) -> Result<Vec<(String, String)>, String> {
        self.search_projection
            .parser_owned_callable_selector_pairs(owner_paths)
    }

    pub fn search_generation_authority(
        &self,
    ) -> &crate::runtime_server_workspace::WorkspaceSearchGenerationAuthority {
        self.search_projection.authority()
    }

    pub fn graph_generation(&self) -> &agent_semantic_search::ResidentGraphGeneration {
        self.search_projection.graph_generation()
    }

    pub fn generation_digest(&self) -> String {
        self.exact_projection.generation_digest().to_owned()
    }

    pub fn root_digest(&self) -> String {
        self.exact_projection.root_digest().to_owned()
    }

    pub fn try_record_read_observation(
        &self,
        surface: &str,
        stage: &str,
        operation_id: &str,
        language_id: Option<&agent_semantic_client_core::LanguageId>,
        requested_projection: &str,
        elapsed_micros: u64,
        budget_micros: u64,
        budget_status: &str,
    ) -> bool {
        let mut observation = RuntimePerformanceObservation::new(
            surface,
            stage,
            elapsed_micros,
            budget_micros,
            budget_status,
        )
        .with_operation_id(operation_id);
        observation.language_id = language_id.map(|value| value.as_str().to_owned());
        observation.generation_digest = Some(self.generation_digest());
        observation.requested_projection = Some(requested_projection.to_owned());
        try_record_to_active_runtime(observation)
    }

    pub const fn work_counters(&self) -> RuntimeResidentReadWorkCounters {
        RuntimeResidentReadWorkCounters {
            provider_process_count: 0,
            scheduler_task_count: 0,
            filesystem_read_count: 0,
            database_read_count: 0,
            socket_operation_count: 0,
        }
    }
}
