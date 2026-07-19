//! Shared lifecycle import for query-free language-harness projections.

use std::path::Path;

use agent_semantic_client_core::{SemanticSchemaId, SemanticSchemaVersion};
use agent_semantic_client_db::{
    CLIENT_DB_SOURCE_INDEX_SCHEMA_ID, CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION, ClientDbEngine,
    ClientDbLanguageProjection, ClientDbLanguageProjectionImportRequest,
    client_db_source_index_generation_id, source_index_import_from_language_projection,
};

/// Result of validating and importing one parser-owned language projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LanguageProjectionImportReport {
    pub reused: bool,
    pub graph_entity_count: usize,
    pub graph_edge_count: usize,
}

/// Import one query-free projection through the shared source-index lifecycle.
pub fn import_language_projection(
    project_root: &Path,
    projection: ClientDbLanguageProjection,
) -> Result<LanguageProjectionImportReport, String> {
    projection.validate()?;
    let db_engine = ClientDbEngine::resolve(project_root)?;
    let client_dir = db_engine.client_dir().to_path_buf();
    let db_session = ClientDbEngine::open_write_session_client_dir(&client_dir)?;
    let schema_id = SemanticSchemaId::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_ID);
    let schema_version = SemanticSchemaVersion::from(CLIENT_DB_SOURCE_INDEX_SCHEMA_VERSION);
    let previous_file_hashes =
        db_session.latest_source_index_file_hashes(project_root, &schema_id, &schema_version)?;
    let registry_fingerprint = language_projection_registry_fingerprint(&projection);
    let import =
        source_index_import_from_language_projection(ClientDbLanguageProjectionImportRequest {
            generation_id: client_db_source_index_generation_id(),
            project_root: project_root.to_path_buf(),
            previous_file_hashes: previous_file_hashes.clone(),
            registry_fingerprint: registry_fingerprint.clone(),
            projection: projection.clone(),
        })?;
    let workspace_snapshot = agent_semantic_artifacts::WorkspaceSnapshot::from_file_hashes(
        import
            .file_hashes
            .iter()
            .map(|file_hash| (file_hash.path.as_str(), file_hash.sha256.as_str())),
    );
    let source_snapshot = workspace_snapshot.evidence(
        agent_semantic_artifacts::SourceSnapshotKind::Filesystem,
        agent_semantic_artifacts::provider_digest(registry_fingerprint.as_bytes()),
    );
    if db_session
        .reusable_source_index_generation(
            project_root,
            &schema_id,
            &schema_version,
            &import.file_hashes,
        )?
        .is_some()
    {
        return Ok(LanguageProjectionImportReport {
            reused: true,
            graph_entity_count: 0,
            graph_edge_count: 0,
        });
    }
    let report = ClientDbEngine::persist_language_projection_read_model_from_client_dir(
        client_dir,
        &import,
        &projection,
        &source_snapshot,
    )?;
    Ok(LanguageProjectionImportReport {
        reused: false,
        graph_entity_count: report.graph_entity_count,
        graph_edge_count: report.graph_edge_count,
    })
}

fn language_projection_registry_fingerprint(projection: &ClientDbLanguageProjection) -> String {
    format!(
        "language-projection:{}:{}:{}:{}",
        projection.language_id,
        projection.harness.harness_id,
        projection.harness.parser_abi,
        projection.harness.selector_dialect,
    )
}
