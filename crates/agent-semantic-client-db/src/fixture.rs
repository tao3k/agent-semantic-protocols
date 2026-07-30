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
    ) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
        let registry = Arc::clone(&self.registry);
        crate::engine::facade::block_on_db_engine_async(async move {
            let project_root = request.import.project_root.clone();
            let session = registry.bootstrap_workspace(&project_root).await?;
            session.commit_source_index_generation(request).await
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
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    SourceIndexFixture::default().commit_source_index_generation(request)
}

pub fn commit_source_index_generation_from_fixture_dir(
    fixture_client_dir: impl AsRef<std::path::Path>,
    request: crate::ClientDbSourceIndexRefreshRequest,
) -> Result<crate::ClientDbSourceIndexRefreshReport, String> {
    let db_path = crate::ClientDbEngine::db_path_for_client_dir(fixture_client_dir);
    crate::engine::facade::block_on_db_engine_async(async move {
        crate::engine::commit_turso_source_index_generation_in_fixture(&db_path, request).await
    })
}
