use std::path::Path;

use std::time::Instant;

use crate::{
    ClientDbEngine, ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexRefreshRequest,
    client_db_source_index_file_count, source_index_import_with_file_hashes,
};
use agent_semantic_client_core::{
    ClientCacheFileHash, ProjectContext, RuntimeProviderProjectionEvidence, SemanticSchemaId,
    SemanticSchemaVersion,
};

use crate::server_source_index::api::source_index_trace;
use crate::server_source_index::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_PROVIDER_ID, SOURCE_INDEX_SCHEMA_ID,
    SOURCE_INDEX_SCHEMA_VERSION,
};
use crate::server_source_index::generation_commit::PreparedSourceIndexGeneration;
use crate::server_source_index::model::SourceIndexScopeFile;

pub(super) struct SourceIndexRefreshContext {
    db_path: std::path::PathBuf,
    schema_id: SemanticSchemaId,
    schema_version: SemanticSchemaVersion,
}

impl SourceIndexRefreshContext {
    pub(super) fn resolve(project_root: &Path) -> Result<Self, String> {
        let project_context = ProjectContext::resolve(project_root)?;
        project_context.require_inside_workspace(project_root)?;
        let db_engine = ClientDbEngine::resolve_for_write(project_root)?;
        Ok(Self {
            db_path: db_engine.db_path().to_path_buf(),
            schema_id: SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION),
        })
    }

    pub(super) async fn prepare_generation_with_runtime_service_async(
        &self,
        runtime: &crate::runtime_search_service::RuntimeSearchServiceHandle,
        request: SourceIndexGenerationRefresh<'_>,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        let changed_owner_paths = request.changed_owner_paths.map(|paths| {
            paths
                .iter()
                .cloned()
                .collect::<std::collections::BTreeSet<_>>()
        });
        let prepared = self
            .prepare_partial_generation_with_runtime_service_async(
                runtime,
                request,
                cancellation.clone(),
            )
            .await?;
        let Some(changed_owner_paths) = changed_owner_paths else {
            return Ok(prepared);
        };
        crate::server_source_index::generation_overlay::complete_generation_from_optional_active_base(
            &self.db_path,
            prepared,
            changed_owner_paths,
        )
        .await
    }

    async fn prepare_partial_generation_with_runtime_service_async(
        &self,
        runtime: &crate::runtime_search_service::RuntimeSearchServiceHandle,
        request: SourceIndexGenerationRefresh<'_>,
        cancellation: crate::runtime_generation_cancellation::GenerationCancellation,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        let trace_started = Instant::now();
        let (file_hashes, workspace_snapshot, source_snapshot, source_blobs, auxiliary_owners) = tokio::select! {
            result = crate::server_source_index::async_snapshot::source_index_snapshot_from_files_async(
                request.index_root,
                request.files,
                request.registry,
                request.provider_registry,
            ) => result?,
            _ = cancellation.cancelled() => {
                return Err("runtime-generation-cancelled: source snapshot cancelled".to_owned());
            }
        };
        let workspace_identity =
            agent_semantic_client_core::state_core::ResolvedState::resolve(request.index_root)?
                .workspace
                .workspace_id
                .to_string();
        let projected_files =
            crate::server_source_index::projection::project_generation_with_runtime_service(
                runtime,
                cancellation,
                request.index_root,
                &workspace_identity,
                request.provider_registry,
                request.files,
                &source_blobs,
                &auxiliary_owners,
            )
            .await?;
        self.prepare_generation_from_snapshot(
            SourceIndexGenerationRefresh {
                index_root: request.index_root,
                files: &projected_files,
                project_resolutions: request.project_resolutions,
                changed_owner_paths: request.changed_owner_paths,
                candidate: request.candidate,
                registry: request.registry,
                provider_registry: request.provider_registry,
            },
            file_hashes,
            workspace_snapshot,
            source_snapshot,
            source_blobs,
            trace_started,
        )
    }

    fn prepare_generation_from_snapshot(
        &self,
        request: SourceIndexGenerationRefresh<'_>,
        file_hashes: Vec<ClientCacheFileHash>,
        workspace_snapshot: agent_semantic_content_identity::WorkspaceSnapshot,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        source_blobs: crate::ClientDbSourceIndexSourceBlobs,
        trace_started: Instant,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        source_index_trace("generation-file-hashes-built", trace_started);
        let generation_id =
            crate::client_db_source_index_generation_id_for_snapshot(&source_snapshot);
        let import = source_index_import_with_file_hashes(
            ClientDbSourceIndexImportAssemblyRequest {
                generation_id,
                project_root: request.index_root.to_path_buf(),
                schema_id: self.schema_id.clone(),
                schema_version: self.schema_version.clone(),
                selector_source: SOURCE_INDEX_PROVIDER_ID.into(),
                file_text_bytes_limit: SOURCE_INDEX_FILE_BYTES_LIMIT,
                registry_fingerprint: request.registry.fingerprint.clone(),
                extra_scope_dirs: request.registry.scope_dirs.iter().cloned().collect(),
                files: request.files.to_vec(),
                source_blobs: source_blobs.clone(),
            },
            file_hashes,
        )?;
        source_index_trace("generation-import-assembled", trace_started);
        let refresh_request = ClientDbSourceIndexRefreshRequest {
            import,
            file_count: client_db_source_index_file_count(request.files.len()),
            source_snapshot: source_snapshot.clone(),
        };
        let workspace_identity =
            agent_semantic_client_core::state_core::ResolvedState::resolve(request.index_root)?
                .workspace
                .workspace_id
                .to_string();
        let materialization =
            crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                workspace_identity,
                &workspace_snapshot,
                &source_snapshot,
                &refresh_request.import,
                &source_blobs,
                request.project_resolutions.to_vec(),
            )?;
        Ok(PreparedSourceIndexGeneration::new(
            self.db_path.clone(),
            refresh_request,
            materialization,
            request.candidate.clone(),
            trace_started,
        ))
    }
}

pub(super) struct SourceIndexGenerationRefresh<'a> {
    pub(super) changed_owner_paths: Option<&'a [String]>,
    pub(super) index_root: &'a Path,
    pub(super) files: &'a [SourceIndexScopeFile],
    pub(super) project_resolutions: &'a [agent_semantic_runtime::AdmittedProjectResolution],
    pub(super) candidate: &'a crate::runtime_server_admission::WorkspaceGenerationCandidateIdentity,
    pub(super) registry: &'a RuntimeProviderProjectionEvidence,
    pub(super) provider_registry: &'a agent_semantic_client_core::RuntimeProviderProjection,
}
