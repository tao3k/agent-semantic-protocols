//! Shared lifecycle import for query-free language-harness projections.

use std::path::Path;

use agent_semantic_client_db::{
    ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    source_index_import_from_language_projection,
};

/// Result of validating and importing one parser-owned language projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct LanguageProjectionImportReport {
    reused: bool,
    node_locator_count: usize,
}

impl LanguageProjectionImportReport {
    /// Whether the existing source-index generation was reused.
    #[must_use]
    pub const fn reused(&self) -> bool {
        self.reused
    }

    /// Number of node locators imported for a new generation.
    #[must_use]
    pub const fn node_locator_count(&self) -> usize {
        self.node_locator_count
    }
}

/// Import one query-free projection through the shared source-index lifecycle.
pub fn import_language_projection(
    project_root: &Path,
    projection: ClientDbLanguageProjection,
) -> Result<LanguageProjectionImportReport, String> {
    import_language_projection_inner(project_root, projection)
}

fn import_language_projection_inner(
    project_root: &Path,
    projection: ClientDbLanguageProjection,
) -> Result<LanguageProjectionImportReport, String> {
    projection.validate()?;
    let registry_fingerprint = language_projection_registry_fingerprint(&projection);
    let source_blobs = agent_semantic_client_db::ClientDbSourceIndexSourceBlobs::from_normalized(
        projection
            .sources()
            .iter()
            .map(|source| {
                let path =
                    agent_semantic_client_db::ClientDbSourceIndexPath::new(source.path.clone());
                let bytes = std::fs::read(project_root.join(&source.path)).map_err(|error| {
                    format!(
                        "failed to capture language projection source {}: {error}",
                        source.path
                    )
                })?;
                Ok((path, bytes))
            })
            .collect::<Result<Vec<_>, String>>()?,
    );
    let prepared =
        source_index_import_from_language_projection(ClientDbLanguageProjectionImportRequest {
            project_root: project_root.to_path_buf(),
            source_blobs: source_blobs.clone(),
            registry_fingerprint: registry_fingerprint.clone(),
            projection: projection.clone(),
        })?;
    let import = prepared.source_index;
    let source_snapshot = prepared.source_snapshot;
    let workspace_identity =
        agent_semantic_client_core::state_core::ResolvedState::resolve(project_root)?
            .workspace
            .workspace_id
            .to_string();
    let materialization =
        agent_semantic_client_db::runtime_server_workspace::
            WorkspaceCanonicalMaterialization::from_source_index(
                workspace_identity,
                &source_snapshot,
                &import,
                &source_blobs,
            )?;
    let report =
        agent_semantic_client_db::workspace_db_ipc::commit_source_index_generation_via_runtime_server(
            agent_semantic_client_db::ClientDbSourceIndexRefreshRequest {
                file_count: import.file_hashes.len().min(u32::MAX as usize) as u32,
                import,
                source_snapshot,
            },
            materialization,
        )?;
    Ok(LanguageProjectionImportReport {
        reused: report.reused_generation,
        node_locator_count: report.owner_count as usize + report.selector_count as usize,
    })
}

fn language_projection_registry_fingerprint(projection: &ClientDbLanguageProjection) -> String {
    format!(
        "language-projection:{}:{}:{}:{}",
        projection.language_id(),
        projection.harness().harness_id(),
        projection.harness().parser_abi(),
        projection.harness().selector_dialect(),
    )
}
