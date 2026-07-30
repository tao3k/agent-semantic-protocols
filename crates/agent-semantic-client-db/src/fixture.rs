//! Reusable in-process fixtures for exercising the resident workspace DB model.

use std::sync::Arc;

/// Load-once fixture backed by the same registry, read leases, and single writer
/// queue used by the resident transport.
#[derive(Clone, Debug, Default)]
pub struct SourceIndexFixture {
    registry: Arc<crate::WorkspaceDbRegistry>,
}

impl SourceIndexFixture {
    pub fn commit_source_index_generation(
        &self,
        request: crate::ClientDbSourceIndexRefreshRequest,
        source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        let registry = Arc::clone(&self.registry);
        let source_blobs = source_blobs.clone();
        crate::engine::facade::block_on_db_engine_async(async move {
            let project_root = request.import.project_root.clone();
            let session = registry.bootstrap_workspace(&project_root).await?;
            let canonical_source_snapshot = canonical_source_snapshot(&request);
            let materialization =
                crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
                    session.workspace_identity(),
                    &canonical_source_snapshot,
                    &request.import,
                    &source_blobs,
                )?;
            session
                .commit_source_index_generation(request, materialization)
                .await
        })
    }

    pub fn counters(&self) -> crate::WorkspaceDbRegistryCounters {
        self.registry.counters()
    }

    pub fn read_source_index(
        &self,
        project_root: std::path::PathBuf,
        indexed_project_root: std::path::PathBuf,
        source_snapshot: agent_semantic_content_identity::SourceSnapshotEvidence,
        query: String,
        language_id: Option<agent_semantic_client_core::LanguageId>,
        limit: u32,
    ) -> Result<crate::ClientDbSourceIndexLookupResult, String> {
        let registry = Arc::clone(&self.registry);
        crate::engine::facade::block_on_db_engine_async(async move {
            let session = registry.bootstrap_workspace(&project_root).await?;
            session
                .read_source_index(
                    &indexed_project_root,
                    &source_snapshot,
                    &query,
                    language_id.as_ref(),
                    limit,
                )
                .await
        })
    }
}

pub fn commit_source_index_generation(
    request: crate::ClientDbSourceIndexRefreshRequest,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    SourceIndexFixture::default().commit_source_index_generation(request, source_blobs)
}

pub fn commit_source_index_generation_from_fixture_dir(
    fixture_client_dir: impl AsRef<std::path::Path>,
    request: crate::ClientDbSourceIndexRefreshRequest,
    source_blobs: &crate::ClientDbSourceIndexSourceBlobs,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    let workspace_identity = request.import.project_root.display().to_string();
    let canonical_source_snapshot = canonical_source_snapshot(&request);
    let materialization =
        crate::runtime_server_workspace::WorkspaceCanonicalMaterialization::from_source_index(
            workspace_identity,
            &canonical_source_snapshot,
            &request.import,
            source_blobs,
        )?;
    commit_source_index_generation_with_materialization_from_fixture_dir(
        fixture_client_dir,
        request,
        materialization,
    )
}

fn canonical_source_snapshot(
    request: &crate::ClientDbSourceIndexRefreshRequest,
) -> agent_semantic_content_identity::SourceSnapshotEvidence {
    agent_semantic_content_identity::WorkspaceSnapshot::from_file_hashes(
        request.import.file_hashes.iter().map(|file| {
            (
                file.path.as_str().to_owned(),
                file.sha256.as_str().to_owned(),
            )
        }),
    )
    .evidence(
        request.source_snapshot.source_kind.clone(),
        request.source_snapshot.provider_digest.clone(),
    )
}

pub fn commit_source_index_generation_with_materialization_from_fixture_dir(
    fixture_client_dir: impl AsRef<std::path::Path>,
    request: crate::ClientDbSourceIndexRefreshRequest,
    materialization: crate::runtime_server_workspace::WorkspaceCanonicalMaterialization,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    let db_path = crate::ClientDbEngine::db_path_for_client_dir(fixture_client_dir);
    crate::engine::facade::block_on_db_engine_async(async move {
        crate::engine::commit_turso_source_index_generation_in_fixture(
            &db_path,
            request,
            materialization,
        )
        .await
    })
}

pub fn load_active_workspace_generation_materialization_from_fixture_dir(
    fixture_client_dir: impl AsRef<std::path::Path>,
    workspace_identity: &str,
) -> Result<Option<crate::runtime_server_workspace::WorkspaceCanonicalMaterialization>, String> {
    let db_path = crate::ClientDbEngine::db_path_for_client_dir(fixture_client_dir);
    let workspace_identity = workspace_identity.to_owned();
    crate::engine::facade::block_on_db_engine_async(async move {
        crate::engine::load_active_workspace_generation_materialization_in_fixture(
            &db_path,
            &workspace_identity,
        )
        .await
    })
}
