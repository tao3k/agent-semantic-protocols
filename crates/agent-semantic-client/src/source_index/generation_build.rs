use std::path::Path;
use std::time::Instant;

use agent_semantic_client_core::{
    ClientCacheFileHash, ProjectContext, ProviderRegistryEvidence, SemanticSchemaId,
    SemanticSchemaVersion,
};
use agent_semantic_client_db::{
    ClientDbEngine, ClientDbSourceIndexImportAssemblyRequest, ClientDbSourceIndexRefreshRequest,
    client_db_source_index_file_count, source_index_import_with_file_hashes,
};

use super::api::{source_index_snapshot_from_files, source_index_trace};
use super::config::{
    SOURCE_INDEX_FILE_BYTES_LIMIT, SOURCE_INDEX_PROVIDER_ID, SOURCE_INDEX_SCHEMA_ID,
    SOURCE_INDEX_SCHEMA_VERSION,
};
use super::generation_commit::PreparedSourceIndexGeneration;
use super::model::{SourceIndexRefreshReport, SourceIndexScopeFile};

pub(super) struct SourceIndexRefreshContext {
    db_path: std::path::PathBuf,
    client_cache_dir: std::path::PathBuf,
    schema_id: SemanticSchemaId,
    schema_version: SemanticSchemaVersion,
}

impl SourceIndexRefreshContext {
    pub(super) fn resolve(project_root: &Path) -> Result<Self, String> {
        let project_context = ProjectContext::resolve(project_root)?;
        project_context.require_inside_workspace(project_root)?;
        let db_engine = ClientDbEngine::resolve(project_root)?;
        Ok(Self {
            db_path: db_engine.db_path().to_path_buf(),
            client_cache_dir: db_engine.client_dir().to_path_buf(),
            schema_id: SemanticSchemaId::from(SOURCE_INDEX_SCHEMA_ID),
            schema_version: SemanticSchemaVersion::from(SOURCE_INDEX_SCHEMA_VERSION),
        })
    }

    pub(super) fn client_cache_dir(&self) -> &Path {
        &self.client_cache_dir
    }

    pub(super) fn refresh_generation(
        &mut self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<SourceIndexRefreshReport, String> {
        self.prepare_generation(request)?.commit()
    }

    pub(super) fn prepare_generation(
        &self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        let trace_started = Instant::now();
        let (file_hashes, _workspace_snapshot, source_snapshot, source_blobs) =
            source_index_snapshot_from_files(request.index_root, request.files, request.registry)?;
        self.prepare_generation_from_snapshot(
            request,
            file_hashes,
            source_snapshot,
            source_blobs,
            trace_started,
        )
    }

    pub(super) async fn prepare_generation_async(
        &self,
        request: SourceIndexGenerationRefresh<'_>,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        let trace_started = Instant::now();
        let (file_hashes, _workspace_snapshot, source_snapshot, source_blobs) =
            super::async_snapshot::source_index_snapshot_from_files_async(
                request.index_root,
                request.files,
                request.registry,
            )
            .await?;
        self.prepare_generation_from_snapshot(
            request,
            file_hashes,
            source_snapshot,
            source_blobs,
            trace_started,
        )
    }

    fn prepare_generation_from_snapshot(
        &self,
        request: SourceIndexGenerationRefresh<'_>,
        file_hashes: Vec<ClientCacheFileHash>,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        source_blobs: agent_semantic_client_db::ClientDbSourceIndexSourceBlobs,
        trace_started: Instant,
    ) -> Result<PreparedSourceIndexGeneration, String> {
        source_index_trace("generation-file-hashes-built", trace_started);
        let generation_id =
            agent_semantic_client_db::client_db_source_index_generation_id_for_snapshot(
                &source_snapshot,
            );
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
            agent_semantic_client_db::runtime_server_workspace::
                WorkspaceCanonicalMaterialization::from_source_index(
                    workspace_identity,
                    &source_snapshot,
                    &refresh_request.import,
                    &source_blobs,
                )?;
        Ok(PreparedSourceIndexGeneration::new(
            self.db_path.clone(),
            refresh_request,
            materialization,
            trace_started,
        ))
    }
}

pub(super) struct SourceIndexGenerationRefresh<'a> {
    pub(super) index_root: &'a Path,
    pub(super) files: &'a [SourceIndexScopeFile],
    pub(super) registry: &'a ProviderRegistryEvidence,
}
